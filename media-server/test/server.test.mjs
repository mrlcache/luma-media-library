import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp,writeFile,mkdir,rm,stat } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import http from 'node:http';
import { DatabaseSync } from 'node:sqlite';
import { XMLParser,XMLValidator } from 'fast-xml-parser';
import { MediaServer,peerAllowed } from '../src/server.mjs';
import { range,xml,envelope,CD,CM,scpd } from '../src/protocol.mjs';
import { safeFile,Catalog } from '../src/catalog.mjs';
import { discoveryMessage,targets } from '../src/ssdp.mjs';
import { Transcoder } from '../src/transcode.mjs';
import { promisify } from 'node:util';
import { execFile } from 'node:child_process';

const parse=new XMLParser({removeNSPrefix:true,parseTagValue:false});
async function fixture(){
  const root=await mkdtemp(path.join(os.tmpdir(),'luma-server-test-'));
  const media=path.join(root,'media');await mkdir(media);await writeFile(path.join(media,'episode.mkv'),'0123456789');await writeFile(path.join(root,'private.mkv'),'secret');
  const dbPath=path.join(root,'library.sqlite3');const db=new DatabaseSync(dbPath);
  db.exec(`CREATE TABLE library_roots(id INTEGER,canonical_path TEXT,current_generation INTEGER);
    CREATE TABLE media_items(id INTEGER,root_id INTEGER,relative_path TEXT,local_title TEXT,local_kind TEXT,local_year INTEGER,local_key TEXT);
    CREATE TABLE media_files(root_id INTEGER,relative_path TEXT,display_name TEXT,extension TEXT,size_bytes INTEGER,generation INTEGER);
    CREATE TABLE media_metadata(media_id INTEGER,title TEXT,kind TEXT,release_year INTEGER,tmdb_id INTEGER,poster_url TEXT,overview TEXT);`);
  db.prepare('INSERT INTO library_roots VALUES(1,?,2)').run(media);
  db.exec(`INSERT INTO media_items VALUES(10,1,'episode.mkv','Filename Title','series',2025,'show');
    INSERT INTO media_items VALUES(11,1,'../private.mkv','Private','movie',2020,'private');
    INSERT INTO media_files VALUES(1,'episode.mkv','Show.S01E01.mkv','mkv',10,2);
    INSERT INTO media_files VALUES(1,'old.mkv','Old','mkv',10,1);
    INSERT INTO media_files VALUES(1,'../private.mkv','Private','mkv',6,2);
    INSERT INTO media_metadata VALUES(10,'API & Title','series',2025,101,'https://image.tmdb.org/t/p/w500/test.jpg','A <story>');`);
  db.close();return {root,media,dbPath};
}
async function soap(base,service,action,args){const urn=service==='ContentDirectory'?CD:CM;return fetch(`${base}/upnp/${service}/control`,{method:'POST',headers:{'Content-Type':'text/xml',SOAPAction:`"${urn}#${action}"`},body:envelope(`<u:${action} xmlns:u="${urn}">${Object.entries(args).map(([k,v])=>`<${k}>${xml(v)}</${k}>`).join('')}</u:${action}>`)});}

test('HTTP range semantics including suffix, empty and invalid requests',()=>{
  assert.deepEqual(range('bytes=2-5',10),{start:2,end:5,partial:true});assert.deepEqual(range('bytes=-3',10),{start:7,end:9,partial:true});assert.deepEqual(range('bytes=6-',10),{start:6,end:9,partial:true});assert.deepEqual(range('bytes=0-100',10),{start:0,end:9,partial:true});
  for(const value of ['bytes=10-','bytes=5-2','bytes=-0','bytes=0-1,3-4','bytes=-','bytes=9999999999999999999-'])assert.equal(range(value,10),null);
  assert.equal(range('bytes=0-',0),null);
});
test('access confined to configured subnet',()=>{assert.ok(peerAllowed('192.168.1.50','192.168.1.3','255.255.255.0'));assert.ok(peerAllowed('::ffff:192.168.1.50','192.168.1.3','255.255.255.0'));assert.equal(peerAllowed('192.168.2.50','192.168.1.3','255.255.255.0'),false);assert.equal(peerAllowed('8.8.8.8','192.168.1.3','255.255.255.0'),false);});
test('SSDP identity, discovery location and shutdown announcement',()=>{const id='123';const location='http://192.168.1.3:8941/description.xml';assert.equal(targets(id).length,5);assert.match(discoveryMessage({uuid:id,location,target:CD,response:true}),/^HTTP\/1.1 200 OK\r\n/);assert.match(discoveryMessage({uuid:id,location,target:CD,response:true}),/USN: uuid:123::urn:schemas/);assert.match(discoveryMessage({uuid:id,location,target:CD,alive:false}),/NTS: ssdp:byebye/);});
test('both service descriptions are well-formed with required actions',()=>{for(const service of ['ContentDirectory','ConnectionManager']){const body=scpd(service);assert.equal(XMLValidator.validate(body),true);assert.match(body,/<serviceStateTable>/);}assert.match(scpd('ContentDirectory'),/<name>Browse<\/name>/);});
test('catalog reads TMDb metadata and rejects escaping paths',async t=>{
  const f=await fixture();t.after(()=>rm(f.root,{recursive:true,force:true}));const catalog=new Catalog(f.dbPath);await catalog.refresh();
  assert.equal(catalog.items.find(i=>i.mediaId===10).title,'API & Title');assert.equal(catalog.items.find(i=>i.mediaId===10).displayTitle,'API & Title · S01E01');assert.equal(catalog.items.length,2);assert.equal(await catalog.file('file-11'),null);await assert.rejects(safeFile(f.media,'../private.mkv'));
  assert.ok(!('root' in catalog.publicItems()[0]));assert.ok(!('relative' in catalog.publicItems()[0]));
});
test('server browses series, streams original ranges, events and stops/restarts',async t=>{
  const f=await fixture();const before=await stat(f.dbPath);const server=new MediaServer({dbPath:f.dbPath,host:'127.0.0.1',netmask:'255.0.0.0',port:0,tools:path.join(f.root,'no-tools'),discoveryEnabled:false});
  t.after(async()=>{await server.stop();await rm(f.root,{recursive:true,force:true});});await server.start();const base=server.baseURL;
  const description=await(await fetch(`${base}/description.xml`)).text();assert.equal(XMLValidator.validate(description),true);assert.match(description,/<friendlyName>Luma/);
  let result=await soap(base,'ContentDirectory','Browse',{ObjectID:'0',BrowseFlag:'BrowseDirectChildren',Filter:'*',StartingIndex:0,RequestedCount:0,SortCriteria:''});assert.equal(result.status,200);let body=parse.parse(await result.text()).Envelope.Body.BrowseResponse;assert.equal(body.NumberReturned,'2');assert.equal(XMLValidator.validate(body.Result),true);
  result=await soap(base,'ContentDirectory','Browse',{ObjectID:'series',BrowseFlag:'BrowseDirectChildren',Filter:'*',StartingIndex:0,RequestedCount:0,SortCriteria:''});body=parse.parse(await result.text()).Envelope.Body.BrowseResponse;assert.match(body.Result,/API &amp; Title/);assert.match(body.Result,/image.tmdb.org/);
  result=await soap(base,'ContentDirectory','Browse',{ObjectID:'missing',BrowseFlag:'BrowseMetadata',Filter:'*',StartingIndex:0,RequestedCount:0,SortCriteria:''});assert.equal(result.status,500);assert.match(await result.text(),/<errorCode>701/);
  result=await soap(base,'ConnectionManager','GetProtocolInfo',{});assert.equal(result.status,200);assert.match(await result.text(),/video\/x-matroska/);
  result=await fetch(`${base}/media/file-10/original`,{headers:{Range:'bytes=2-5'}});assert.equal(result.status,206);assert.equal(result.headers.get('Content-Range'),'bytes 2-5/10');assert.equal(await result.text(),'2345');
  result=await fetch(`${base}/media/file-10/original`,{method:'HEAD'});assert.equal(result.headers.get('Content-Length'),'10');assert.equal(await result.text(),'');
  assert.equal((await fetch(`${base}/media/file-10/original`,{headers:{Range:'bytes=10-'}})).status,416);
  assert.equal((await fetch(`${base}/media/file-11/original`)).status,404);
  assert.equal((await fetch(`${base}/media/file-10/compatible`)).status,503);
  let notifyResolve;const notified=new Promise(resolve=>{notifyResolve=resolve;});const callback=http.createServer((req,res)=>{let b='';req.on('data',c=>b+=c);req.on('end',()=>{notifyResolve({method:req.method,seq:req.headers.seq,body:b});res.end();});});await new Promise(resolve=>callback.listen(0,'127.0.0.1',resolve));t.after(()=>new Promise(resolve=>callback.close(resolve)));
  result=await fetch(`${base}/upnp/ContentDirectory/event`,{method:'SUBSCRIBE',headers:{NT:'upnp:event',CALLBACK:`<http://127.0.0.1:${callback.address().port}/event>`}});assert.equal(result.status,200);const sid=result.headers.get('SID');
  const event=await Promise.race([notified,new Promise((_,reject)=>setTimeout(()=>reject(Error('No event')),2000))]);assert.equal(event.method,'NOTIFY');assert.equal(event.seq,'0');assert.match(event.body,/<SystemUpdateID>1/);
  assert.equal((await fetch(`${base}/upnp/ContentDirectory/event`,{method:'UNSUBSCRIBE',headers:{SID:sid}})).status,200);
  assert.equal((await fetch(`${base}/upnp/ContentDirectory/event`,{method:'SUBSCRIBE',headers:{NT:'upnp:event',CALLBACK:'<http://8.8.8.8/event>'}})).status,412);
  await server.stop();assert.equal(server.running,false);await assert.rejects(fetch(`${base}/description.xml`));await server.start();assert.equal(server.running,true);await server.stop();assert.equal((await stat(f.dbPath)).mtimeMs,before.mtimeMs);
});
test('malformed XML and entity definitions get SOAP faults',async t=>{
  const f=await fixture();const server=new MediaServer({dbPath:f.dbPath,host:'127.0.0.1',netmask:'255.0.0.0',port:0,tools:path.join(f.root,'no-tools'),discoveryEnabled:false});t.after(async()=>{await server.stop();await rm(f.root,{recursive:true,force:true});});await server.start();
  for(const body of ['<bad>','<!DOCTYPE e [<!ENTITY secret SYSTEM "file:///secret">]><e>&secret;</e>']){const result=await fetch(`${server.baseURL}/upnp/ContentDirectory/control`,{method:'POST',headers:{SOAPAction:`"${CD}#Browse"`},body});assert.equal(result.status,500);assert.match(await result.text(),/<errorCode>402/);}
});
test('FFmpeg compatible stream preserves already compatible codecs via remux',async t=>{
  const tools=path.resolve('tools');const transcoder=new Transcoder(tools);if(!transcoder.available){t.skip('Run setup-ffmpeg.ps1 for conversion integration test');return;}
  const f=await fixture();t.after(()=>rm(f.root,{recursive:true,force:true}));
  const video=path.join(f.media,'episode.mkv');await promisify(execFile)(transcoder.ffmpeg,['-y','-v','error','-f','lavfi','-i','color=c=black:s=160x90:r=24','-f','lavfi','-i','anullsrc=r=48000:cl=stereo','-t','1','-c:v','libx264','-pix_fmt','yuv420p','-c:a','aac',video],{windowsHide:true,timeout:20000});
  const server=new MediaServer({dbPath:f.dbPath,host:'127.0.0.1',netmask:'255.0.0.0',port:0,tools,discoveryEnabled:false});t.after(()=>server.stop());await server.start();
  const result=await fetch(`${server.baseURL}/media/file-10/compatible`);assert.equal(result.status,200);assert.equal(result.headers.get('X-Luma-Playback'),'remux');const bytes=new Uint8Array(await result.arrayBuffer());assert.equal(bytes[0],0x47);assert.ok(bytes.length>188);
});
test('FFmpeg converts incompatible audio only and incompatible video when requested',async t=>{
  const tools=path.resolve('tools');const transcoder=new Transcoder(tools);if(!transcoder.available){t.skip('Run setup-ffmpeg.ps1');return;}
  const f=await fixture();const server=new MediaServer({dbPath:f.dbPath,host:'127.0.0.1',netmask:'255.0.0.0',port:0,tools,discoveryEnabled:false});
  t.after(async()=>{await server.stop();await rm(f.root,{recursive:true,force:true});});
  const video=path.join(f.media,'episode.mkv');const exec=promisify(execFile);
  for(const [codec,expected] of [['libx264','audio-transcode'],['mpeg4','transcode']]){
    await exec(transcoder.ffmpeg,['-y','-v','error','-f','lavfi','-i','color=c=black:s=160x90:r=24','-f','lavfi','-i','anullsrc=r=48000:cl=stereo','-t','1','-c:v',codec,'-pix_fmt','yuv420p','-c:a','pcm_s16le',video],{windowsHide:true,timeout:20000});
    await server.start();const result=await fetch(`${server.baseURL}/media/file-10/compatible`);assert.equal(result.status,200);assert.equal(result.headers.get('X-Luma-Playback'),expected);const data=new Uint8Array(await result.arrayBuffer());assert.ok(data.length>188);assert.equal(data[0],0x47);await server.stop();
  }
});
test('FFmpeg emits fragmented MP4 with H.264/AAC, capped dimensions and duration metadata',async t=>{
  const tools=path.resolve('tools');const transcoder=new Transcoder(tools);if(!transcoder.available){t.skip('Run setup-ffmpeg.ps1');return;}
  const f=await fixture();t.after(()=>rm(f.root,{recursive:true,force:true}));const exec=promisify(execFile);
  const source=path.join(f.media,'episode.mkv');
  await exec(transcoder.ffmpeg,['-y','-v','error','-f','lavfi','-i','color=c=black:s=1600x900:r=24','-f','lavfi','-i','anullsrc=r=48000:cl=stereo','-t','2','-c:v','mpeg4','-q:v','5','-c:a','pcm_s16le',source],{windowsHide:true,timeout:30000});
  const catalog=new Catalog(f.dbPath);await catalog.refresh(true);const file=await catalog.file('file-10');assert.ok(file);
  const server=http.createServer((req,res)=>transcoder.stream(file,req,res,0,'mp4'));
  t.after(()=>new Promise(resolve=>server.close(resolve)));await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const response=await fetch(`http://127.0.0.1:${server.address().port}/stream`);assert.equal(response.status,200);assert.equal(response.headers.get('Content-Type'),'video/mp4');assert.equal(response.headers.get('Access-Control-Expose-Headers'),'X-Luma-Duration');assert.ok(Number(response.headers.get('X-Luma-Duration'))>=1.9);
  const output=path.join(f.root,'converted.mp4');await writeFile(output,Buffer.from(await response.arrayBuffer()));
  const {stdout}=await exec(transcoder.ffprobe,['-v','error','-show_entries','stream=codec_name,width,height','-of','json',output],{windowsHide:true,timeout:15000});const streams=JSON.parse(stdout).streams;
  assert.ok(streams.some(stream=>stream.codec_name==='h264'&&stream.width===1280&&stream.height===720));assert.ok(streams.some(stream=>stream.codec_name==='aac'));
});
