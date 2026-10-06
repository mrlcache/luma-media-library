import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';

const source = readFileSync(new URL('../src/lib/components/PlayerOverlay.svelte', import.meta.url), 'utf8');
const loader = source.slice(source.indexOf('\tasync function loadMobileStream('), source.indexOf('\tasync function retryMobilePlayback('));
const timeline = ts.transpileModule(readFileSync(new URL('../src/lib/platform/playback-timeline.ts', import.meta.url), 'utf8'), {compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
const timelineExports={};new Function('exports',timeline)(timelineExports);
function fixture(fetch) {
  const code = `
    let mobileLoadAttempt=0, mobileMetadataAbort, playbackError='', isLoading=false;
    let compatibleStream;
    let transcoding=true, activeTranscoding=false, transcodeQuality='720p', transcodeBitrate=2500000;
    let mobileSourceUrl='http://localhost/api/v1/media/1?token=fixture', mobileStreamSession='fixture';
    let duration=0, sourceBitrate=0, playerDisposed=false, mediaReady=true, isPlaying=true;
    let transcodeOffset=0, resumePosition=0, mobilePendingSeek=null, mobilePendingResume=false, currentTime=0, playbackRate=1, controlsVisible=false;
    const video={src:'',pause(){},removeAttribute(name){if(name==='src')this.src='';},load(){},async play(){}};
    function stopCompatibleStream(){compatibleStream?.stop();compatibleStream=undefined;}
    function withTimeout(promise){return promise;}
    function fetchCompatibleMedia(url,options,stage){return fetch(url,options,stage);}
    function openHlsStream(video,url){video.src=url;return {ready:Promise.resolve(),stop(){},suspend(){}};}
    ${loader}
    return {loadMobileStream, setRequested(value){transcoding=value;},
      state(){return {activeTranscoding,playbackError,src:video.src,transcodeOffset};}};
  `;
  return new Function('fetch','resumePlaybackPosition', ts.transpileModule(code, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText)(fetch,timelineExports.resumePlaybackPosition);
}
test('indicator stays attached to the loaded stream even if requested mode changes during preparation', async () => {
  let complete;
  const f = fixture(() => new Promise(resolve => { complete=resolve; }));
  const loading = f.loadMobileStream(60);
  assert.equal(f.state().activeTranscoding, false);
  f.setRequested(false);
  complete({ok:true,headers:new Headers({'X-Luma-Duration':'600'})});
  await loading;
  assert.equal(f.state().activeTranscoding, true);
  assert.equal(f.state().transcodeOffset, 60);
  assert.match(f.state().src, /hls\/index\.m3u8/, f.state().playbackError);
});

test('restores a valid saved position and restarts invalid completed resume records instead of jumping to the end', async () => {
  const f=fixture(async()=>({ok:true,headers:new Headers({'X-Luma-Duration':'600'})}));
  await f.loadMobileStream(120,true);
  assert.equal(new URL(f.state().src).searchParams.get('start'),'120');
  await f.loadMobileStream(599,true);
  assert.equal(new URL(f.state().src).searchParams.get('start'),'0');
  await f.loadMobileStream(599,false);
  assert.equal(new URL(f.state().src).searchParams.get('start'),'599');
});

function progressFixture() {
  const handlers=source.slice(source.indexOf('\tfunction onTimeUpdate()'),source.indexOf('\tfunction recordPlaybackStarted()'));
  const ended=source.slice(source.indexOf('\tasync function handlePlaybackEnded()'),source.indexOf('\tasync function signInToOpenSubtitles()'));
  const code=`
    let media={id:'11'}, video={currentTime:15,duration:480}, activeEngine=null, activeTranscoding=true;
    let transcodeOffset=120,currentTime=120,duration=600,lastSavedPosition=120,isLoading=false,mediaReady=true,playbackError='';
    let autoplayTransitioning=false,isPlaying=true,playbackActivityPromise=null,nativePoll=null,playerDisposed=false;
    const saves=[];let continueWatching=120;
    async function savePlaybackProgress(id,position,length){saves.push({id,position,length});continueWatching=position<10||position>=length-20||position/length>=.95?null:position;}
    function readPlaybackPreferences(){return {autoplayNextEpisode:false};}
    ${handlers}${ended}
    return {onTimeUpdate,persistProgress,handlePlaybackEnded,saves,
      set(value){if(value.loading!==undefined)isLoading=value.loading;if(value.ready!==undefined)mediaReady=value.ready;if(value.error!==undefined)playbackError=value.error;if(value.time!==undefined)video.currentTime=value.time;},
      state(){return {currentTime,continueWatching,playbackError};}};
  `;
  return new Function('playbackPosition','reachedPlaybackEnd',ts.transpileModule(code,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText)(timelineExports.playbackPosition,timelineExports.reachedPlaybackEnd);
}
test('early end keeps Continue Watching at the decoded position, never marking the whole episode watched',async()=>{
  const f=progressFixture();await f.handlePlaybackEnded();
  assert.deepEqual(f.saves,[{id:11,position:135,length:600}]);
  assert.equal(f.state().continueWatching,135);
  assert.match(f.state().playbackError,/stopped before/);
});
test('source reset, failed open and unready frames cannot erase existing progress',async()=>{
  const f=progressFixture();
  f.set({loading:true,time:480});f.onTimeUpdate();await f.persistProgress();await f.handlePlaybackEnded();
  f.set({loading:false,ready:false});await f.persistProgress();await f.handlePlaybackEnded();
  f.set({ready:true,error:'File missing'});await f.persistProgress();await f.handlePlaybackEnded();
  assert.deepEqual(f.saves,[]);assert.equal(f.state().continueWatching,120);
});
test('normal completion still removes completed episodes from Continue Watching',async()=>{
  const f=progressFixture();f.set({time:480});await f.handlePlaybackEnded();
  assert.equal(f.state().currentTime,600);assert.equal(f.state().continueWatching,null);
});

test('dragging the mobile timeline only starts one stream at the released position',()=>{
  const handlers=source.slice(source.indexOf('\tfunction seekToPercent('),source.indexOf('\tfunction onTimeUpdate()'));
  const code=`
    let duration=600,previewOnly=false,mobilePlayer=true,activeTranscoding=true,activeEngine=null;
    let currentTime=120,scrubPercent=null;const video={currentTime:0};const calls=[];
    function loadMobileStream(position){calls.push(position);}
    function revealControls(){}
    ${handlers}
    return {previewSeek,commitSeek,calls};
  `;
  const f=new Function(ts.transpileModule(code,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText)();
  for(const value of [20,40,60,80])f.previewSeek({currentTarget:{value:String(value)}});
  assert.deepEqual(f.calls,[]);
  f.commitSeek({currentTarget:{value:'80'}});
  assert.deepEqual(f.calls,[480]);
});
test('a failed HLS stream change reports the failure without changing the active transcode indicator', async () => {
  let fail=false;
  const f = fixture(async () => { if(fail) throw new Error('fixture failure'); return {ok:true,headers:new Headers()}; });
  await f.loadMobileStream();
  fail=true;
  await f.loadMobileStream(120);
  assert.equal(f.state().activeTranscoding,true);
  assert.equal(f.state().playbackError,'fixture failure');
});
