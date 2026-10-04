import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';

const source = readFileSync(new URL('../src/lib/components/PlayerOverlay.svelte', import.meta.url), 'utf8');
const loader = source.slice(source.indexOf('\tasync function loadMobileStream('), source.indexOf('\tasync function retryMobilePlayback('));
function fixture(fetch) {
  const code = `
    let mobileLoadAttempt=0, mobileMetadataAbort, playbackError='', isLoading=false;
    let transcoding=true, activeTranscoding=false, transcodeQuality='720p', transcodeBitrate=2500000;
    let mobileSourceUrl='http://localhost/api/v1/media/1?token=fixture', mobileStreamSession='fixture';
    let duration=0, sourceBitrate=0, playerDisposed=false, mediaReady=true, isPlaying=true;
    let transcodeOffset=0, resumePosition=0, mobilePendingSeek=null, currentTime=0, playbackRate=1, controlsVisible=false;
    const video={src:'',pause(){},removeAttribute(){},load(){},async play(){}};
    ${loader}
    return {loadMobileStream, setRequested(value){transcoding=value;},
      state(){return {activeTranscoding,playbackError,src:video.src,transcodeOffset};}};
  `;
  return new Function('fetch', ts.transpileModule(code, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText)(fetch);
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
  assert.match(f.state().src, /compatible/);
});
test('a failed stream change keeps the indicator consistent with the previous stream', async () => {
  let fail=false;
  const f = fixture(async () => { if(fail) throw new Error('fixture failure'); return {ok:true,headers:new Headers()}; });
  await f.loadMobileStream();
  const previous=f.state().src;
  fail=true;
  await f.loadMobileStream(120);
  assert.equal(f.state().activeTranscoding,true);
  assert.equal(f.state().src,previous);
  assert.equal(f.state().playbackError,'fixture failure');
});
