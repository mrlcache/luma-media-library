import http from 'node:http';
import { createReadStream } from 'node:fs';
import { networkInterfaces } from 'node:os';
import { randomUUID } from 'node:crypto';
import { pipeline } from 'node:stream';
import { Catalog } from './catalog.mjs';
import { Discovery } from './ssdp.mjs';
import { Transcoder } from './transcode.mjs';
import { CD,CM,description,scpd,parseAction,response,fault,didl,range,xml } from './protocol.mjs';

function ipv4Number(ip) { const parts=ip.split('.'); if(parts.length!==4 || parts.some(p=>!/^\d+$/.test(p)||Number(p)>255))return null;return parts.reduce((n,p)=>(n*256+Number(p))>>>0,0); }
export function lanInterfaces() { return Object.entries(networkInterfaces()).flatMap(([name,addresses])=>addresses.filter(a=>a.family === 'IPv4'&&!a.internal&&!a.address.startsWith('169.254.')).map(a=>({...a,name}))); }
export function peerAllowed(ip, host, netmask) {
  ip = ip.replace(/^::ffff:/,'');
  if(ip === '127.0.0.1') return true;
  const a=ipv4Number(ip),h=ipv4Number(host),m=ipv4Number(netmask);
  return a !== null && h !== null && m !== null && ((a&m)>>>0) === ((h&m)>>>0);
}
export async function readBody(req) { let data='';for await(const chunk of req){data+=chunk;if(Buffer.byteLength(data)>65536)throw new Error('Request too large');}return data; }
const send = (res,status,body,type='text/xml; charset=utf-8',extra={}) => {res.writeHead(status,{'Content-Type':type,'Content-Length':Buffer.byteLength(body),...extra});res.end(body);};
function protocolInfo(catalog,transcoder) { return [...new Set(catalog.items.map(i=>`http-get:*:${i.mime}:*`)),...(transcoder.available?['http-get:*:video/mp2t:*']:[])].join(','); }

export class MediaServer {
  constructor({dbPath,host,netmask,port=8941,uuid=randomUUID(),name='Luma · Server preview',tools,discoveryEnabled=true}) {
    Object.assign(this,{host,netmask,port,uuid,name,discoveryEnabled});
    this.catalog=new Catalog(dbPath);this.transcoder=new Transcoder(tools);this.running=false;this.subscriptions=new Map();this.lastError=null;this.sockets=new Set();
  }
  get baseURL() {return `http://${this.host}:${this.port}`;}
  status() {return {running:this.running,name:this.name,host:this.host,port:this.port,url:this.baseURL,items:this.catalog.items.length,updateId:this.catalog.updateId,transcoding:this.transcoder.available,activeTranscodes:this.transcoder.active.size,error:this.lastError || this.discovery?.lastError || this.transcoder.lastError};}
  async start() {
    if(this.running)return this.status();
    await this.catalog.refresh(true);
    const server=http.createServer((req,res)=>this.handle(req,res).catch(error=>{this.lastError=error.message;if(!res.headersSent)send(res,500,'Request failed','text/plain');else res.destroy();}));
    server.requestTimeout=30000;server.headersTimeout=10000;
    server.on('connection',socket=>{this.sockets.add(socket);socket.on('close',()=>this.sockets.delete(socket));});
    await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(this.port,this.host,()=>{server.removeListener('error',reject);resolve();});});
    this.http=server;this.port=server.address().port;
    server.on('error',error=>{this.lastError=error.message;});
    this.discovery=new Discovery({uuid:this.uuid,host:this.host,location:`${this.baseURL}/description.xml`,allowPeer:ip=>peerAllowed(ip,this.host,this.netmask)});
    try {if(this.discoveryEnabled)await this.discovery.start();}catch(error){await this.closeHTTP();throw error;}
    this.running=true;this.lastError=null;
    this.interval=setInterval(()=>this.refresh().catch(error=>{this.lastError=error.message;}),5000);
    return this.status();
  }
  async refresh() {const before=this.catalog.updateId;await this.catalog.refresh();if(before!==this.catalog.updateId)for(const sub of this.subscriptions.values())if(sub.service==='ContentDirectory')this.notify(sub);}
  async closeHTTP() {if(!this.http)return;const server=this.http;this.http=null;await new Promise(resolve=>{server.close(resolve);for(const socket of this.sockets)socket.destroy();});}
  async stop() {clearInterval(this.interval);this.transcoder.stop();await this.discovery?.stop();this.subscriptions.clear();await this.closeHTTP();this.running=false;return this.status();}
  async handle(req,res) {
    if(!peerAllowed(req.socket.remoteAddress,this.host,this.netmask)){send(res,403,'Local network only','text/plain');return;}
    const url=new URL(req.url,this.baseURL);
    if(url.pathname === '/api/library' && req.method === 'GET') {
      await this.catalog.refresh();
      const items=this.catalog.publicItems().map(item=>({...item,original:`${this.baseURL}/media/${item.id}/original`,compatible:this.transcoder.available?`${this.baseURL}/media/${item.id}/compatible`:null}));
      return send(res,200,JSON.stringify({items,updateId:this.catalog.updateId}),'application/json');
    }
    if(url.pathname === '/description.xml' && ['GET','HEAD'].includes(req.method)){const body=description(this.uuid,this.name,this.baseURL);if(req.method==='HEAD'){res.writeHead(200,{'Content-Type':'text/xml','Content-Length':Buffer.byteLength(body)});res.end();}else send(res,200,body);return;}
    const servicePath=/^\/upnp\/(ContentDirectory|ConnectionManager)\/(scpd.xml|control|event)$/.exec(url.pathname);
    if(servicePath){const [,service,endpoint]=servicePath;if(endpoint==='scpd.xml'&&req.method==='GET')return send(res,200,scpd(service));if(endpoint==='control'&&req.method==='POST')return this.control(req,res,service);if(endpoint==='event'&&['SUBSCRIBE','UNSUBSCRIBE'].includes(req.method))return this.subscribe(req,res,service);return send(res,405,'Method not allowed','text/plain');}
    const media=/^\/media\/(file-\d+)\/(original|compatible)$/.exec(url.pathname);
    if(media && ['GET','HEAD'].includes(req.method)) {
      const file=await this.catalog.file(media[1]);if(!file)return send(res,404,'Media not found','text/plain');
      if(media[2]==='compatible'){
        if(!this.transcoder.available)return send(res,503,'FFmpeg unavailable','text/plain');
        const start=Number(url.searchParams.get('start') || 0);
        if(!Number.isFinite(start)||start<0||start>86400)return send(res,400,'Invalid start position','text/plain');
        if(req.headers.range)return send(res,416,'Use a start position for converted media','text/plain');
        if(req.method==='HEAD'){res.writeHead(200,{'Content-Type':'video/mp2t'});res.end();return;}
        try {return await this.transcoder.stream(file,req,res,start);}catch(error){return send(res,422,error.message,'text/plain');}
      }
      const selected=range(req.headers.range,file.info.size);
      if(!selected)return send(res,416,'Range not satisfiable','text/plain',{'Content-Range':`bytes */${file.info.size}`});
      const length=file.info.size === 0 ? 0 : selected.end-selected.start+1;
      const headers={'Content-Type':file.mime,'Content-Length':length,'Accept-Ranges':'bytes','Last-Modified':file.info.mtime.toUTCString(),'transferMode.dlna.org':'Streaming','contentFeatures.dlna.org':'DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000','X-Luma-Playback':'direct'};
      if(selected.partial)headers['Content-Range']=`bytes ${selected.start}-${selected.end}/${file.info.size}`;
      res.writeHead(selected.partial?206:200,headers);
      if(req.method==='HEAD'||length===0){res.end();return;}
      pipeline(createReadStream(file.path,{start:selected.start,end:selected.end}),res,()=>{});return;
    }
    send(res,404,'Not found','text/plain');
  }
  async control(req,res,service) {
    const urn=service==='ContentDirectory'?CD:CM;
    let action;
    try {action=parseAction(await readBody(req),req.headers.soapaction,urn);}catch{return send(res,500,fault(402,'Invalid Args'));}
    await this.catalog.refresh();
    const {name,args}=action;
    if(service==='ConnectionManager'){
      if(name==='GetProtocolInfo')return send(res,200,response(urn,name,{Source:protocolInfo(this.catalog,this.transcoder),Sink:''}));
      if(name==='GetCurrentConnectionIDs')return send(res,200,response(urn,name,{ConnectionIDs:'0'}));
      if(name==='GetCurrentConnectionInfo'){
        if(String(args.ConnectionID)!=='0')return send(res,500,fault(706,'Invalid Connection Reference'));
        return send(res,200,response(urn,name,{RcsID:-1,AVTransportID:-1,ProtocolInfo:'',PeerConnectionManager:'',PeerConnectionID:-1,Direction:'Output',Status:'OK'}));
      }
    }else{
      if(name==='GetSearchCapabilities')return send(res,200,response(urn,name,{SearchCaps:''}));
      if(name==='GetSortCapabilities')return send(res,200,response(urn,name,{SortCaps:'dc:title'}));
      if(name==='GetSystemUpdateID')return send(res,200,response(urn,name,{Id:this.catalog.updateId}));
      if(name==='Browse'){
        if(!['BrowseDirectChildren','BrowseMetadata'].includes(args.BrowseFlag)||!/^\d+$/.test(args.StartingIndex??'')||!/^\d+$/.test(args.RequestedCount??'')||args.ObjectID===undefined||args.Filter===undefined||args.SortCriteria===undefined)return send(res,500,fault(402,'Invalid Args'));
        const offset=Number(args.StartingIndex),count=Number(args.RequestedCount);
        if(!Number.isSafeInteger(offset)||!Number.isSafeInteger(count)||offset>0xffffffff||count>0xffffffff)return send(res,500,fault(402,'Invalid Args'));
        const sort=String(args.SortCriteria);
        if(sort && !['+dc:title','-dc:title'].includes(sort))return send(res,500,fault(709,'Unsupported or invalid sort criteria'));
        let items=this.catalog.browse(String(args.ObjectID),args.BrowseFlag==='BrowseMetadata');
        if(!items)return send(res,500,fault(701,'No such object'));
        if(sort)items.sort((a,b)=>(a.displayTitle||a.title).localeCompare(b.displayTitle||b.title)*(sort[0]==='-'?-1:1));
        const total=items.length;
        if(args.BrowseFlag !== 'BrowseMetadata')items=items.slice(offset,count===0?undefined:offset+count);
        return send(res,200,response(urn,name,{Result:didl(items,this.baseURL,this.transcoder.available),NumberReturned:items.length,TotalMatches:total,UpdateID:this.catalog.updateId}));
      }
    }
    send(res,500,fault(401,'Invalid Action'));
  }
  subscribe(req,res,service) {
    for(const [id,sub] of this.subscriptions)if(sub.expires<Date.now())this.subscriptions.delete(id);
    const sid=req.headers.sid;
    if(req.method==='UNSUBSCRIBE'){
      const sub=this.subscriptions.get(sid);if(!sub||sub.service!==service||sub.peer!==req.socket.remoteAddress){res.writeHead(412);res.end();return;}
      this.subscriptions.delete(sid);res.writeHead(200);res.end();return;
    }
    let sub=sid?this.subscriptions.get(sid):null;
    if(sid){if(!sub||sub.service!==service||sub.peer!==req.socket.remoteAddress||req.headers.callback||req.headers.nt){res.writeHead(412);res.end();return;}}
    else{
      if(req.headers.nt!=='upnp:event'||this.subscriptions.size>=64){res.writeHead(412);res.end();return;}
      try {
        const match=/^<([^<>]+)>$/.exec(req.headers.callback || '');if(!match)throw new Error();
        const callback=new URL(match[1]);const peer=req.socket.remoteAddress.replace(/^::ffff:/,'');
        if(callback.protocol!=='http:'||callback.hostname!==peer||callback.username||callback.password||callback.hash)throw new Error();
        sub={sid:`uuid:${randomUUID()}`,service,peer:req.socket.remoteAddress,callback,seq:0};this.subscriptions.set(sub.sid,sub);
      }catch{res.writeHead(412);res.end();return;}
    }
    sub.expires=Date.now()+300000;
    res.writeHead(200,{'SID':sub.sid,'TIMEOUT':'Second-300'});res.end();
    if(!sid)setTimeout(()=>this.notify(sub),50);
  }
  notify(sub) {
    if(sub.expires<Date.now()||!this.subscriptions.has(sub.sid))return;
    const values=sub.service==='ContentDirectory'?{SystemUpdateID:this.catalog.updateId}:{SourceProtocolInfo:protocolInfo(this.catalog,this.transcoder),SinkProtocolInfo:'',CurrentConnectionIDs:'0'};
    const body=`<?xml version="1.0"?><e:propertyset xmlns:e="urn:schemas-upnp-org:event-1-0">${Object.entries(values).map(([key,value])=>`<e:property><${key}>${xml(value)}</${key}></e:property>`).join('')}</e:propertyset>`;
    const request=http.request(sub.callback,{method:'NOTIFY',timeout:3000,headers:{'Content-Type':'text/xml; charset=utf-8','Content-Length':Buffer.byteLength(body),'NT':'upnp:event','NTS':'upnp:propchange','SID':sub.sid,'SEQ':sub.seq++}},reply=>reply.resume());
    request.on('error',()=>{});request.on('timeout',()=>request.destroy());request.end(body);
  }
}
