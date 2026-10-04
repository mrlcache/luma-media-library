import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFileSync,existsSync,mkdirSync,writeFileSync } from 'node:fs';
import { randomUUID,createHash } from 'node:crypto';
import { MediaServer,lanInterfaces } from './server.mjs';
import { createReleaseSearch } from '../../scripts/torrent-search-preview.mjs';
const releaseSearch = createReleaseSearch();

const workspace=path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const runtime=path.resolve(process.env.LUMA_SERVER_RUNTIME||path.join(workspace,'.runtime'));mkdirSync(runtime,{recursive:true});
const options={};
for(let i=2;i<process.argv.length;i++){const arg=process.argv[i];if(arg.startsWith('--'))options[arg.slice(2)]=process.argv[i+1]?.startsWith('--')||!process.argv[i+1]?true:process.argv[++i];}
const interfaces=lanInterfaces();
const selected=options.host?interfaces.find(i=>i.address===options.host):interfaces.find(i=>i.name.toLowerCase().includes('wi'))||interfaces[0];
if(!selected)throw new Error('No local IPv4 network interface. Pass --host with an active local address.');
const dbPath=path.resolve(options.db||process.env.LUMA_SERVER_DB||path.join(process.env.LUMA_APP_DATA||path.join(process.env.APPDATA||'','local.media.platform'),'media-library.sqlite3'));
if(!existsSync(dbPath))throw new Error('Luma library not found. Pass --db with the library database path.');
const identityFile=path.join(runtime,'identity.json');
const uuid=existsSync(identityFile)?JSON.parse(readFileSync(identityFile,'utf8')).uuid:randomUUID();
writeFileSync(identityFile,JSON.stringify({uuid}));
const media=new MediaServer({dbPath,host:selected.address,netmask:selected.netmask,port:Number(options.port||8941),uuid,tools:path.join(workspace,'tools')});
await media.catalog.refresh(true);
let busy=false;
const stateFile=path.join(runtime,'server-state.json');
const savedState=existsSync(stateFile)?JSON.parse(readFileSync(stateFile,'utf8')):{enabled:false};
const adminPort=Number(options['admin-port']||8940);
const origin=`http://127.0.0.1:${adminPort}`;
const page=readFileSync(path.join(workspace,'web','index.html'));
const devVersion=createHash('sha256').update(page).update(String(Date.now())).digest('hex');
const admin=http.createServer(async(req,res)=>{
  try{
    // Host check prevents DNS rebinding; mutation headers and Origin reject browser CSRF.
    if(req.headers.host!==`127.0.0.1:${adminPort}`){res.writeHead(403);res.end();return;}
    const url=new URL(req.url,origin);
    if(url.pathname==='/'&&req.method==='GET'){res.writeHead(200,{'Content-Type':'text/html; charset=utf-8','Cache-Control':'no-store','Content-Security-Policy':"default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' https://image.tmdb.org data:; object-src 'none'; frame-ancestors 'none'"});res.end(page);return;}
    if(url.pathname==='/manrope.woff2'&&req.method==='GET'){const font=path.join(workspace,'web','manrope.woff2');if(existsSync(font)){res.writeHead(200,{'Content-Type':'font/woff2'});res.end(readFileSync(font));return;}}
    const mobileStream=/^\/api\/mobile-stream\/(file-\d+)$/.exec(url.pathname);
    if(mobileStream&&['GET','HEAD'].includes(req.method)){
      if(req.headers['x-luma-control']!=='1'){res.writeHead(403);res.end();return;}
      const start=Number(url.searchParams.get('start')||0);
      if(!Number.isFinite(start)||start<0||start>86400){res.writeHead(400,{'Content-Type':'text/plain'});res.end('Invalid start position');return;}
      const identity=req.headers['x-luma-media-identity'];
      const file=typeof identity==='string' ? await media.catalog.matchingFile(mobileStream[1],identity) : await media.catalog.file(mobileStream[1]);
      if(!file){res.writeHead(404,{'Content-Type':'text/plain'});res.end('Media not found');return;}
      if(!media.transcoder.available){res.writeHead(503,{'Content-Type':'text/plain'});res.end('FFmpeg unavailable');return;}
      if(req.method==='HEAD'){
        const plan=await media.transcoder.plan(file,'mp4');
        res.writeHead(200,{'Content-Type':'video/mp4','X-Luma-Duration':String(plan.duration),'Cache-Control':'no-store','Access-Control-Allow-Origin':'*','Access-Control-Expose-Headers':'X-Luma-Duration'});res.end();return;
      }
      return await media.transcoder.stream(file,req,res,start,'mp4');
    }
    let result;
    if(url.pathname==='/api/dev-version'&&req.method==='GET')result={version:devVersion,enabled:Boolean(options.dev)};
    else if(url.pathname==='/api/releases'&&req.method==='GET') {
      if(req.headers['x-luma-control']!=='1'){res.writeHead(403);res.end();return;}
      result=await releaseSearch(url.searchParams);
    }
    else if(url.pathname==='/api/status'&&req.method==='GET')result=media.status();
    else if(url.pathname==='/api/library'&&req.method==='GET'){await media.catalog.refresh();result=media.catalog.publicItems();}
    else if(['/api/start','/api/stop'].includes(url.pathname)&&req.method==='POST'){
      if(req.headers.origin!==origin||req.headers['x-luma-control']!=='1'){res.writeHead(403);res.end();return;}
      if(busy){res.writeHead(409);res.end();return;}
      busy=true;try{
        result=await(url.pathname==='/api/start'?media.start():media.stop());
        writeFileSync(stateFile,JSON.stringify({enabled:media.status().running}));
      }finally{busy=false;}
    }else{res.writeHead(404);res.end();return;}
    res.writeHead(200,{'Content-Type':'application/json','Cache-Control':'no-store'});res.end(JSON.stringify(result));
  }catch(error){res.writeHead(500,{'Content-Type':'application/json'});res.end(JSON.stringify({error:error.message}));}
});
await new Promise((resolve,reject)=>{admin.once('error',reject);admin.listen(adminPort,'127.0.0.1',resolve);});
writeFileSync(path.join(runtime,'process.json'),JSON.stringify({pid:process.pid,admin:origin,media:media.baseURL,dbPath,workspace}));
if(options.start||savedState.enabled)try{await media.start();}catch(error){media.lastError=error.message;console.error('Media server startup:',error.message);}
console.log(JSON.stringify({admin:origin,...media.status()}));
let shuttingDown=false;
async function shutdown(){if(shuttingDown)return;shuttingDown=true;await media.stop();admin.close(()=>process.exit(0));}
process.on('SIGINT',shutdown);process.on('SIGTERM',shutdown);
