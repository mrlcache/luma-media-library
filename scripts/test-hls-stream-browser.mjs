import assert from 'node:assert/strict';
import { mkdtemp, readFile, stat, rm } from 'node:fs/promises';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import ts from 'typescript';
import { Transcoder } from '../media-server/src/transcode.mjs';
import { MobileHls } from '../media-server/src/mobile-hls.mjs';

const root = await mkdtemp(path.join(os.tmpdir(), 'luma-hls-browser-'));
const transcoder = new Transcoder(path.resolve('media-server/tools'));
let browser, server, socket, hls;
let failedOnce = false, segmentGets = 0, stoppedSessions = 0;
try {
  const realMedia = Boolean(process.env.LUMA_TEST_MEDIA);
  const offset = Number(process.env.LUMA_TEST_OFFSET || 5);
  const input = process.env.LUMA_TEST_MEDIA || path.join(root, 'source.mkv');
  if (!realMedia) await promisify(execFile)(transcoder.ffmpeg, ['-y','-v','error','-f','lavfi','-i','testsrc2=s=320x180:r=24','-f','lavfi','-i','anullsrc=r=48000:cl=stereo','-t','18','-c:v','libx264','-g','48','-c:a','aac',input], { windowsHide:true, timeout:30000 });
  const file = { id:'file-11', path:input, info:await stat(input) };
  const normalized = input.replace(/^\\\\\?\\/, '').replaceAll('\\','/').toLowerCase();
  const identity = createHash('sha256').update(normalized).digest('hex');
  const media = { transcoder, catalog: { matchingFile:async (_, expected) => expected === identity ? file : null } };
  hls = new MobileHls({ media, workspace:root });
  const helper = ts.transpileModule(await readFile(new URL('../src/lib/platform/hls-stream.ts',import.meta.url),'utf8'), {compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText.replace(/^export /gm,'').replace("import('hls.js')", "import('/hls.mjs')");
  const html = `<video id="video" muted playsinline></video><script>${helper}
    window.result=(async()=>{const video=document.querySelector('video');let error='';
      const stream=openHlsStream(video,location.origin+'/api/v1/media/11/hls/index.m3u8?start=${offset}&quality=480p&bitrate=1200000&session=browser-test',e=>error=e.message);
      await stream.ready;await video.play();await new Promise(r=>setTimeout(r,8500));
      const result={time:video.currentTime,error,buffered:video.buffered.length};video.pause();stream.suspend();await new Promise(r=>setTimeout(r,500));stream.stop();return result;})();</script>`;
  const module = await readFile('node_modules/hls.js/dist/hls.mjs');
  server = http.createServer(async (req,res) => {
    try {
      if(req.url==='/hls.mjs'){res.writeHead(200,{'Content-Type':'text/javascript'});res.end(module);return;}
      if(req.url.startsWith('/api/v1/media/11/hls/')) {
        if(/segment-\d+\.ts/.test(req.url)&&req.method==='GET') {
          segmentGets++;
          if (!failedOnce) {failedOnce=true;res.writeHead(503);res.end('Temporary segment failure');return;}
        }
        req.headers['x-luma-control']='1'; req.headers['x-luma-media-identity']=identity;
        const upstream=new URL(req.url,'http://127.0.0.1');
        upstream.pathname=upstream.pathname.replace('/api/v1/media/11/hls/index.m3u8','/api/mobile-hls/file-11').replace('/api/v1/media/11/hls/session/','/api/mobile-hls/session/');
        if(req.method==='DELETE'){stoppedSessions++;upstream.pathname=upstream.pathname.replace(/\/index\.m3u8$/,'');}
        // Mirror the bridge's authenticated playlist rewrite in this loopback fixture.
        const originalHead=res.writeHead.bind(res); let status=200;
        res.writeHead=(code,headers)=>{status=code;if(headers&&typeof headers==='object')for(const [key,value] of Object.entries(headers))res.setHeader(key,value);return res;};
        const originalEnd=res.end.bind(res);
        res.end=(body,...args)=>{
          if (body && String(res.getHeader('Content-Type')).includes('mpegurl')) {
            const session=res.getHeader('X-Luma-Hls-Session');
            body=String(body).split('\n').map(line=>/^segment-\d+\.ts$/.test(line)?`/api/v1/media/11/hls/session/${session}/${line}`:line).join('\n');
            res.setHeader('Content-Length',Buffer.byteLength(body));
          }
          originalHead(status);
          return originalEnd(body,...args);
        };
        if(!await hls.handle(req,res,upstream)) {res.writeHead(404);res.end();}
        return;
      }
      res.writeHead(200,{'Content-Type':'text/html'});res.end(html);
    } catch(error) { if(!res.headersSent)res.writeHead(500);res.end(error.message); }
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  const profile=path.join(root,'browser');
  browser=spawn('C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',['--headless=new','--disable-gpu','--no-first-run','--no-default-browser-check','--autoplay-policy=no-user-gesture-required','--remote-debugging-port=0',`--user-data-dir=${profile}`,`http://127.0.0.1:${server.address().port}/`],{windowsHide:true,stdio:'ignore'});
  let port;
  for(let i=0;i<100;i++){try{port=Number((await readFile(path.join(profile,'DevToolsActivePort'),'utf8')).split('\n')[0]);break;}catch{await new Promise(r=>setTimeout(r,100));}}
  assert.ok(port,'Headless browser must start');
  const targets=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  socket=new WebSocket(targets.find(t=>t.type==='page').webSocketDebuggerUrl);
  await new Promise((resolve,reject)=>{socket.onopen=resolve;socket.onerror=reject;});
  const result=await new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>reject(new Error('HLS browser playback timed out')),45000);
    socket.onmessage=event=>{const message=JSON.parse(event.data);if(message.id===1){clearTimeout(timer);if(message.result?.exceptionDetails)reject(new Error(JSON.stringify(message.result.exceptionDetails)));else resolve(message.result.result.value);}};
    socket.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{expression:'window.result',awaitPromise:true,returnByValue:true}}));
  });
  assert.equal(result.error,''); assert.ok(result.time>7,JSON.stringify(result));
  assert.ok(failedOnce && segmentGets>=3,'Playback must survive one failed segment request');
  await new Promise(r=>setTimeout(r,500));
  assert.equal(stoppedSessions,1,'Closing the player must stop its HLS session');
  assert.equal(transcoder.active.size,0,'Closing the player must release the encoder');
  console.log('HLS playback, segment retry and encoder cleanup passed:',{...result,segmentGets});
} finally {
  if(socket?.readyState===WebSocket.OPEN)socket.send(JSON.stringify({id:99,method:'Browser.close'}));
  await new Promise(r=>setTimeout(r,500));socket?.close();browser?.kill();transcoder.stop();
  await hls?.close();server?.closeAllConnections();if(server)await new Promise(r=>server.close(r));
  assert.ok(path.resolve(root).startsWith(path.resolve(os.tmpdir())+path.sep));
  await rm(root,{recursive:true,force:true,maxRetries:3,retryDelay:250}).catch(()=>{});
}
