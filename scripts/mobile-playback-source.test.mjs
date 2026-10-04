import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';

// Run the real resolver with native IPC replaced by an isolated library fixture.
const source=readFileSync(new URL('../src/lib/platform/desktop.ts',import.meta.url),'utf8');
const resolver=source.slice(source.indexOf('export async function resolveMediaFile('),source.indexOf('export async function savePlaybackProgress('))
  .replace("const { invoke } = await import('$lib/platform/invoke');",'const invoke = mockInvoke;');
const code=ts.transpileModule(resolver,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
function fixture(duplicate=false){
  const correct={tmdbId:101,media:{id:11,title:'MobLand',kind:'series'},files:[{mediaId:11,season:1,episode:1},{mediaId:12,season:1,episode:2},...(duplicate?[{mediaId:21,season:1,episode:1}]:[])]};
  const wrong={tmdbId:202,media:{id:9,title:'Marvel Zombies',kind:'series'},files:[{mediaId:9,season:1,episode:3}]};
  const calls=[];const exports={};let invalidations=0;
  const invoke=async(command,args)=>{
    calls.push([command,args]);
    if(command==='get_local_title_detail')return args.mediaId===9?wrong:correct;
    if(command==='get_catalog_page')return {items:[correct.media],total:1,offset:0};
    if(command==='resolve_media_file')return {path:`file-${args.mediaId}.mp4`,subtitles:[],resumePositionSeconds:30};
    throw new Error(`Unexpected command: ${command}`);
  };
  new Function('exports','nativeMobile','isDesktopRuntime','invalidateCatalogPageCache','mockInvoke',code)(exports,true,()=>true,()=>invalidations++,invoke);
  return {...exports,calls,invalidations:()=>invalidations};
}
test('stale MobLand ID cannot play Marvel Zombies and recovers the matching episode',async()=>{
  const f=fixture();const resolved=await f.resolveMediaFile(9,'MobLand','S01E01 · Stick or Twist',101);
  assert.equal(resolved.mediaId,11);assert.equal(resolved.path,'file-11.mp4');assert.equal(f.invalidations(),1);
  assert.deepEqual(f.calls.filter(([command])=>command==='resolve_media_file').map(([,args])=>args.mediaId),[11]);
});
test('a stale ID for another episode in the same series recovers the requested episode',async()=>{
  const f=fixture();const resolved=await f.resolveMediaFile(11,'MobLand','S01E02 · Jigsaw Puzzle',101);
  assert.equal(resolved.mediaId,12);
});
test('ambiguous replacement versions are rejected without opening any file',async()=>{
  const f=fixture(true);await assert.rejects(f.resolveMediaFile(9,'MobLand','S01E01',101),/Couldn’t identify/);
  assert.equal(f.calls.some(([command])=>command==='resolve_media_file'),false);
});
