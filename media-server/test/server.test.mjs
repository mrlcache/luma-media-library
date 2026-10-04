import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp,writeFile,mkdir,rm,stat } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import http from 'node:http';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { DatabaseSync } from 'node:sqlite';
import { XMLParser,XMLValidator } from 'fast-xml-parser';
import { MediaServer,peerAllowed } from '../src/server.mjs';
import { range,xml,envelope,CD,CM,scpd } from '../src/protocol.mjs';
import { safeFile,Catalog,fileIdentity } from '../src/catalog.mjs';
import { discoveryMessage,targets } from '../src/ssdp.mjs';
import { Transcoder } from '../src/transcode.mjs';
import { promisify } from 'node:util';
import { execFile } from 'node:child_process';

const parse=new XMLParser({removeNSPrefix:true,parseTagValue:false});
const mediaServerRoot=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
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
test('catalog matches the real file when another catalog reuses its numeric ID',async t=>{
  const f=await fixture();t.after(()=>rm(f.root,{recursive:true,force:true}));
  await writeFile(path.join(f.media,'other.mkv'),'another show');
  const db=new DatabaseSync(f.dbPath);
  db.exec("INSERT INTO media_items VALUES(12,1,'other.mkv','Other Show','series',2025,'other'); INSERT INTO media_files VALUES(1,'other.mkv','Other.S01E01.mkv','mkv',12,2);");db.close();
  const catalog=new Catalog(f.dbPath);await catalog.refresh(true);
  const expected=await catalog.file('file-12');
  assert.equal((await catalog.matchingFile('file-10',fileIdentity(expected.path))).id,'file-12');
  assert.equal(await catalog.matchingFile('file-10','0'.repeat(64)),null);
  assert.equal(await catalog.matchingFile('file-10',''),null);
  assert.equal(fileIdentity('C:\\Media\\Episode.mkv'),fileIdentity('\\\\?\\C:\\Media\\Episode.mkv'));
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
test('mobile admin stream echoes media identity and applies successive quality and bitrate choices',async t=>{
  const tools=path.join(mediaServerRoot,'tools');const transcoder=new Transcoder(tools);if(!transcoder.available){t.skip('Run setup-ffmpeg.ps1 for conversion integration test');return;}
  const host=Object.values(os.networkInterfaces()).flat().find(address=>address?.family==='IPv4'&&!address.internal&&!address.address.startsWith('169.254.'))?.address;
  if(!host){t.skip('A local IPv4 interface is required to launch the media server');return;}
  const f=await fixture();t.after(()=>rm(f.root,{recursive:true,force:true}));
  const exec=promisify(execFile);const source=path.join(f.media,'episode.mkv');
  await exec(transcoder.ffmpeg,['-y','-v','error','-f','lavfi','-i','testsrc2=size=1600x900:rate=24','-f','lavfi','-i','anullsrc=r=48000:cl=5.1','-t','2','-c:v','libx265','-pix_fmt','yuv420p10le','-x265-params','pools=1:frame-threads=1','-c:a','eac3',source],{windowsHide:true,timeout:30000});
  const portProbe=http.createServer();await new Promise((resolve,reject)=>{portProbe.once('error',reject);portProbe.listen(0,'127.0.0.1',resolve);});const adminPort=portProbe.address().port;await new Promise(resolve=>portProbe.close(resolve));
  const runtime=path.join(f.root,'runtime');
  const child=spawn(process.execPath,[path.join(mediaServerRoot,'src','main.mjs'),'--host',host,'--admin-port',String(adminPort),'--db',f.dbPath],{cwd:mediaServerRoot,windowsHide:true,stdio:['ignore','pipe','pipe'],env:{...process.env,LUMA_SERVER_RUNTIME:runtime,LUMA_FFMPEG:transcoder.ffmpeg,LUMA_FFPROBE:transcoder.ffprobe}});
  let stdout='',stderr='',pending='';child.stdout.setEncoding('utf8');child.stderr.setEncoding('utf8');
  const ready=new Promise((resolve,reject)=>{
    child.stdout.on('data',chunk=>{stdout+=chunk;pending+=chunk;const lines=pending.split('\n');pending=lines.pop();for(const line of lines){try{const state=JSON.parse(line);if(state.admin)resolve(state);}catch{}}});
    child.stderr.on('data',chunk=>{stderr+=chunk;});
    child.once('error',reject);
    child.once('exit',code=>reject(new Error(`Media server exited before readiness (${code}): ${stderr||stdout}`)));
  });
  t.after(async()=>{if(child.exitCode!==null)return;const exited=new Promise(resolve=>child.once('exit',resolve));child.kill();await Promise.race([exited,new Promise(resolve=>setTimeout(resolve,3000))]);if(child.exitCode===null)child.kill('SIGKILL');});
  let readyTimeout;const state=await Promise.race([ready,new Promise((_,reject)=>{readyTimeout=setTimeout(()=>reject(new Error(`Timed out waiting for media server: ${stderr||stdout}`)),10000);})]);clearTimeout(readyTimeout);
  const identity=fileIdentity(source);const session='quality-switch-01';const headers={'X-Luma-Control':'1','X-Luma-Media-Identity':identity};
  const invalid=await fetch(`${state.admin}/api/mobile-stream/file-10?quality=480p&bitrate=800000&session=bad%2Fsession`,{method:'HEAD',headers});assert.equal(invalid.status,400);
  const invalidBitrate=await fetch(`${state.admin}/api/mobile-stream/file-10?quality=480p&bitrate=900000&session=${session}`,{headers});assert.equal(invalidBitrate.status,500);assert.equal(invalidBitrate.headers.get('X-Luma-Media-Identity'),identity);assert.match(await invalidBitrate.text(),/Unsupported bitrate for this quality/);
  for(const choice of [{quality:'480p',bitrate:800000,width:854,height:480},{quality:'720p',bitrate:4000000,width:1280,height:720}]){
    const url=`${state.admin}/api/mobile-stream/file-10?quality=${choice.quality}&bitrate=${choice.bitrate}&session=${session}`;
    const head=await fetch(url,{method:'HEAD',headers});assert.equal(head.status,200);assert.equal(head.headers.get('X-Luma-Media-Identity'),identity);assert.equal(head.headers.get('X-Luma-Bitrate-Limit'),String(choice.bitrate));assert.ok(Number(head.headers.get('X-Luma-Source-Bitrate'))>0);assert.ok(Number(head.headers.get('X-Luma-Duration'))>=1.9);assert.equal((await head.arrayBuffer()).byteLength,0);
    const response=await fetch(url,{headers});assert.equal(response.status,200);assert.equal(response.headers.get('X-Luma-Media-Identity'),identity);assert.ok(Number(response.headers.get('X-Luma-Duration'))>=1.9);
    const output=path.join(f.root,`${choice.quality}.mp4`);await writeFile(output,Buffer.from(await response.arrayBuffer()));assert.ok((await stat(output)).size>1000);
    const {stdout:probe}=await exec(transcoder.ffprobe,['-v','error','-show_entries','stream=codec_name,width,height:format=duration,bit_rate','-of','json',output],{windowsHide:true,timeout:15000});const result=JSON.parse(probe);const video=result.streams.find(stream=>stream.codec_name==='h264');assert.ok(video);assert.equal(video.width,choice.width);assert.equal(video.height,choice.height);assert.ok(Number(result.format.duration)>=1.9);assert.ok(Number(result.format.bit_rate)>0);
  }
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
  const server=http.createServer((req,res)=>transcoder.stream(file,req,res,0,'mp4',new URL(req.url,'http://localhost').searchParams.get('quality')||'auto'));
  t.after(()=>new Promise(resolve=>server.close(resolve)));await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const response=await fetch(`http://127.0.0.1:${server.address().port}/stream`);assert.equal(response.status,200);assert.equal(response.headers.get('Content-Type'),'video/mp4');assert.equal(response.headers.get('Access-Control-Expose-Headers'),'X-Luma-Duration');assert.ok(Number(response.headers.get('X-Luma-Duration'))>=1.9);
  const output=path.join(f.root,'converted.mp4');await writeFile(output,Buffer.from(await response.arrayBuffer()));
  const {stdout}=await exec(transcoder.ffprobe,['-v','error','-show_entries','stream=codec_name,width,height','-of','json',output],{windowsHide:true,timeout:15000});const streams=JSON.parse(stdout).streams;
  assert.ok(streams.some(stream=>stream.codec_name==='h264'&&stream.width===1280&&stream.height===720));assert.ok(streams.some(stream=>stream.codec_name==='aac'));
  const low=await transcoder.plan(file,'mp4','480p');assert.equal(low.profile.bitrate,1_200_000);assert.equal(low.copyVideo,false);
  await assert.rejects(transcoder.plan(file,'mp4','invalid'),/Unsupported quality/);
  const lowResponse=await fetch(`http://127.0.0.1:${server.address().port}/stream?quality=480p`);
  const lowOutput=path.join(f.root,'480p.mp4');await writeFile(lowOutput,Buffer.from(await lowResponse.arrayBuffer()));
  const {stdout:lowProbe}=await exec(transcoder.ffprobe,['-v','error','-show_entries','stream=codec_name,width,height','-of','json',lowOutput],{windowsHide:true,timeout:15000});
  assert.ok(JSON.parse(lowProbe).streams.some(stream=>stream.codec_name==='h264'&&stream.width<=854&&stream.height<=480));
});
