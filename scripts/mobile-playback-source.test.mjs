import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';

// Run the real resolver with native IPC replaced by an isolated library fixture.
const source=readFileSync(new URL('../src/lib/platform/desktop.ts',import.meta.url),'utf8');
const resolver=source.slice(source.indexOf('export async function resolveMediaFile('),source.indexOf('export async function savePlaybackProgress('))
  .replace("const { invoke } = await import('$lib/platform/invoke');",'const invoke = mockInvoke;');
const code=ts.transpileModule(resolver,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
const helpers=ts.transpileModule(readFileSync(new URL('../src/lib/platform/library-playback.ts',import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
const helperExports={};new Function('exports',helpers)(helperExports);
function fixture(duplicate=false,{missing=false,title='MobLand',kind='series'}={}){
  const correct={tmdbId:101,media:{id:11,title,kind},files:[{mediaId:11,season:1,episode:1},{mediaId:12,season:1,episode:2},...(duplicate?[{mediaId:21,season:1,episode:1}]:[])]};
  const wrong={tmdbId:202,media:{id:9,title:'Marvel Zombies',kind:'series'},files:[{mediaId:9,season:1,episode:3}]};
  const calls=[];const exports={};let invalidations=0;
  const invoke=async(command,args)=>{
    calls.push([command,args]);
    if(command==='get_local_title_detail')return args.mediaId===9?wrong:correct;
    if(command==='get_catalog_page')return {items:[correct.media],total:1,offset:0};
    if(command==='resolve_media_file'){
      if(missing)throw 'Could not open the indexed media file: The system cannot find the path specified.';
      const id=args.playbackUuid==='file-11-uuid'?11:args.playbackUuid==='file-12-uuid'?12:args.mediaId;
      if(args.playbackUuid && !['file-11-uuid','file-12-uuid'].includes(args.playbackUuid))throw 'This media file is no longer in the library';
      return {mediaId:id,playbackUuid:args.playbackUuid,path:`file-${id}.mp4`,subtitles:[],resumePositionSeconds:30};
    }
    throw new Error(`Unexpected command: ${command}`);
  };
  new Function('exports','nativeMobile','isDesktopRuntime','invalidateCatalogPageCache','mockInvoke','identifyLibraryPlayback','verifyPlaybackIdentity',code)(exports,true,()=>true,()=>invalidations++,invoke,helperExports.identifyLibraryPlayback,helperExports.verifyPlaybackIdentity);
  return {...exports,calls,invalidations:()=>invalidations};
}
test('the MobLand UUID resolves its exact file despite an unrelated numeric ID',async()=>{
  const f=fixture();const resolved=await f.resolveMediaFile(9,'MobLand','S01E01 · Stick or Twist',101,'series','file-11-uuid');
  assert.equal(resolved.mediaId,11);assert.equal(resolved.path,'file-11.mp4');
  assert.equal(f.calls.some(([command])=>command==='get_catalog_page'),false);
});

test('an absent Full Circle file reports the missing computer file without clearing progress or resolving a different title',async()=>{
  const f=fixture(false,{missing:true,title:'Full Circle',kind:'movie'});
  await assert.rejects(f.resolveMediaFile(11,'Full Circle',undefined,101),/file for Full Circle is missing/);
  assert.deepEqual(f.calls.filter(([command])=>command==='resolve_media_file').map(([,args])=>args.mediaId),[11]);
  assert.equal(f.calls.some(([command])=>command==='save_playback_progress'),false);
});
test('Full Circle and The Haunting of Julia resolve by the same TMDB identity regardless of display title',async()=>{
  const f=fixture(false,{title:'Full Circle',kind:'movie'});
  const source=await f.resolveMediaFile(11,'The Haunting of Julia',undefined,101,'movie');
  assert.equal(source.mediaId,11);assert.equal(source.path,'file-11.mp4');
  assert.equal(f.calls.some(([command])=>command==='get_catalog_page'),false);
});
test('a UUID selects the requested episode without guessing from a stale numeric ID',async()=>{
  const f=fixture();const resolved=await f.resolveMediaFile(11,'MobLand','S01E02 · Jigsaw Puzzle',101,'series','file-12-uuid');
  assert.equal(resolved.mediaId,12);
});

test('legacy mismatched episodes are rejected rather than automatically playing another file',async()=>{
  const f=fixture();await assert.rejects(f.resolveMediaFile(11,'MobLand','S01E02 · Jigsaw Puzzle',101),/Couldn’t identify/);
  assert.equal(f.calls.some(([command])=>command==='resolve_media_file'),false);
});

test('a removed UUID is never retried by title or numeric ID',async()=>{
  const f=fixture();await assert.rejects(f.resolveMediaFile(11,'MobLand','S01E01',101,'series','removed-uuid'),/missing/);
  assert.equal(f.calls.length,1);
  assert.equal(f.calls[0][0],'resolve_media_file');
});
test('ambiguous replacement versions are rejected without opening any file',async()=>{
  const f=fixture(true);await assert.rejects(f.resolveMediaFile(9,'MobLand','S01E01',101),/Couldn’t identify/);
  assert.equal(f.calls.some(([command])=>command==='resolve_media_file'),false);
});
