import assert from 'node:assert/strict';
import {mkdtemp,readFile,writeFile,stat,rm} from 'node:fs/promises';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import ts from 'typescript';
import {createHash} from 'node:crypto';
import {Transcoder} from '../media-server/src/transcode.mjs';

const root=await mkdtemp(path.join(os.tmpdir(),'luma-stream-browser-'));
const transcoder=new Transcoder(path.resolve('media-server/tools'));
const exec=promisify(execFile);let browser,server,socket;let gets=0;
try {
  const realMedia = Boolean(process.env.LUMA_TEST_MEDIA);
  const offset = Number(process.env.LUMA_TEST_OFFSET || 5);
  const input=process.env.LUMA_TEST_MEDIA || path.join(root,'source.mkv');
  if (!realMedia) await exec(transcoder.ffmpeg,['-y','-v','error','-f','lavfi','-i','testsrc2=s=320x180:r=24','-f','lavfi','-i','anullsrc=r=48000:cl=stereo','-t','12','-c:v','libx264','-g','48','-c:a','aac',input],{windowsHide:true,timeout:30000});
  const file={path:input,info:await stat(input)};
  const plan=await transcoder.plan(file,'mp4','480p',0,offset);
  const helper=ts.transpileModule(await readFile(new URL('../src/lib/platform/compatible-stream.ts',import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText.replace(/^export /gm,'');
  const html=`<video id="video" muted playsinline></video><script>${helper}
    window.result=(async()=>{const video=document.querySelector('video');let error='',ended=false;video.onended=()=>ended=true;
    const stream=openCompatibleStream(video,'/stream',${plan.duration-offset},e=>error=e.message);
    await stream.ready;await video.play();await new Promise(r=>setTimeout(r,3000));
    const early={time:video.currentTime,duration:video.duration,error,ended};
    await new Promise(r=>setTimeout(r,5000));
    const result={...early,finished:ended,finalTime:video.currentTime};video.pause();stream.stop();return result;})();</script>`;
  const packagedPolicy=JSON.parse(await readFile(new URL('../mobile/src-tauri/tauri.conf.json',import.meta.url),'utf8')).app.security.csp;
  assert.match(packagedPolicy,/media-src[^;]*\bblob:/,'Packaged policy must allow the MediaSource object URL');
  const fixtureScript=html.slice(html.indexOf('<script>')+8,html.lastIndexOf('</script>'));
  const scriptHash=createHash('sha256').update(fixtureScript).digest('base64');
  const fixturePolicy=packagedPolicy.replace("script-src 'self'",`script-src 'self' 'sha256-${scriptHash}'`);
  server=http.createServer((req,res)=>{
    if(req.url==='/stream'){gets++;transcoder.stream(file,req,res,offset,'mp4','480p',0,'fixture').catch(e=>{res.writeHead(500);res.end(e.message);});}
    else{res.writeHead(200,{'Content-Type':'text/html','Content-Security-Policy':fixturePolicy});res.end(html);}
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  const profile=path.join(root,'profile');
  browser=spawn('C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',['--headless=new','--disable-gpu','--no-first-run','--no-default-browser-check','--autoplay-policy=no-user-gesture-required','--remote-debugging-port=0',`--user-data-dir=${profile}`,`http://127.0.0.1:${server.address().port}/`],{windowsHide:true,stdio:'ignore'});
  let port;
  for(let i=0;i<100;i++){try{port=Number((await readFile(path.join(profile,'DevToolsActivePort'),'utf8')).split('\n')[0]);break;}catch{await new Promise(r=>setTimeout(r,100));}}
  assert.ok(port,'Headless browser must start');
  const targets=await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  socket=new WebSocket(targets.find(t=>t.type==='page').webSocketDebuggerUrl);
  await new Promise((resolve,reject)=>{socket.onopen=resolve;socket.onerror=reject;});
  const result=await new Promise((resolve,reject)=>{
    const timeout=setTimeout(()=>reject(new Error('Browser playback timed out')),20000);
    socket.onmessage=event=>{const msg=JSON.parse(event.data);if(msg.id===1){clearTimeout(timeout);if(msg.result?.exceptionDetails)reject(new Error(JSON.stringify(msg.result.exceptionDetails)));else resolve(msg.result.result.value);}};
    socket.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{expression:'window.result',awaitPromise:true,returnByValue:true}}));
  });
  assert.equal(result.error,'');assert.equal(result.ended,false);assert.ok(result.time>2,JSON.stringify(result));assert.ok(Math.abs(result.duration-(plan.duration-offset))<.15,JSON.stringify(result));
  if (!realMedia) {assert.equal(result.finished,true);assert.ok(Math.abs(result.finalTime-result.duration)<.15);}
  else {assert.equal(result.finished,false);assert.ok(result.finalTime>7,JSON.stringify(result));}
  assert.equal(gets,1);
  console.log('Real Chromium playback passed: resumed stream plays beyond first fragments, full duration, one GET.',result);
} finally {
  if(socket?.readyState===WebSocket.OPEN)socket.send(JSON.stringify({id:99,method:'Browser.close'}));
  await new Promise(r=>setTimeout(r,500));
  socket?.close();transcoder.stop();browser?.kill();server?.closeAllConnections();
  if(server)await new Promise(r=>server.close(r));
  await new Promise(r=>setTimeout(r,500));
  assert.ok(path.resolve(root).startsWith(path.resolve(os.tmpdir())+path.sep));
  await rm(root,{recursive:true,force:true,maxRetries:3,retryDelay:250}).catch(()=>{});
}
