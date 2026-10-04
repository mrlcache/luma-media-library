import {readFile,writeFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
const root=process.cwd();
const assets=path.join(root,'mobile/demo-assets');
await mkdir(assets,{recursive:true});
const token=(await readFile(path.join(process.env.APPDATA,'local.media.platform','tmdb_read_access_token'),'utf8')).trim();
const headers={Authorization:`Bearer ${token}`};
async function prepareLogo(item) {
  const kind=item.kind==='series'?'tv':'movie';
  const response=await fetch(`https://api.themoviedb.org/3/${kind}/${item.tmdbId}/images?include_image_language=en,pt,null`,{headers});
  if(!response.ok)throw new Error(`Demo logo metadata request failed: ${response.status}`);
  const rank=logo=>logo.iso_639_1==='en'?3:logo.iso_639_1==='pt'?2:logo.iso_639_1==null?1:0;
  const logos=(await response.json()).logos.filter(logo=>logo.file_path.toLowerCase().endsWith('.png'));
  logos.sort((a,b)=>rank(b)-rank(a)||b.vote_average-a.vote_average||b.width-a.width);
  if(!logos.length) { item.logoUrl=null; return; }
  const image=await fetch(`https://image.tmdb.org/t/p/w500${logos[0].file_path}`);
  if(!image.ok)throw new Error(`Demo logo request failed: ${image.status}`);
  await writeFile(path.join(assets,`${item.id}-logo.png`),Buffer.from(await image.arrayBuffer()));
  item.logoUrl=`/mobile-demo/${item.id}-logo.png`;
}
if(process.argv.includes('--logos-only')) {
  const fixturePath=path.join(root,'src/lib/platform/mobile-demo-fixtures.json');
  const fixtures=JSON.parse(await readFile(fixturePath,'utf8'));
  for(const item of fixtures)await prepareLogo(item);
  await writeFile(fixturePath,JSON.stringify(fixtures,null,2));
  console.log(`Prepared ${fixtures.filter(item=>item.logoUrl).length} bundled PNG logos; existing demo titles preserved.`);
  process.exit(0);
}
const items=[];
for(const kind of ['tv','movie']) {
  const response=await fetch(`https://api.themoviedb.org/3/trending/${kind}/week?language=en-US`,{headers});
  if(!response.ok)throw new Error(`Demo metadata request failed: ${response.status}`);
  for(const raw of (await response.json()).results.slice(0,12)) {
    const id=items.length+1;
    for(const [type,size,source] of [['poster','w500',raw.poster_path],['backdrop','w1280',raw.backdrop_path]]) {
      if(!source)continue;
      const image=await fetch(`https://image.tmdb.org/t/p/${size}${source}`);
      if(!image.ok)throw new Error(`Demo artwork request failed: ${image.status}`);
      await writeFile(path.join(assets,`${id}-${type}.jpg`),Buffer.from(await image.arrayBuffer()));
    }
    items.push({id,title:raw.title||raw.name,tmdbId:raw.id,kind:kind==='tv'?'series':'movie',year:Number((raw.release_date||raw.first_air_date||'').slice(0,4))||2025,overview:raw.overview,voteAverage:raw.vote_average,posterUrl:`/mobile-demo/${id}-poster.jpg`,backdropUrl:raw.backdrop_path?`/mobile-demo/${id}-backdrop.jpg`:`/mobile-demo/${id}-poster.jpg`,extension:'mp4',sizeBytes:2_000_000_000,modifiedAt:Date.now()/1000-id*3600});
  }
}
for(const item of items)await prepareLogo(item);
await writeFile(path.join(root,'src/lib/platform/mobile-demo-fixtures.json'),JSON.stringify(items,null,2));
console.log(`Prepared ${items.length} demo titles with bundled artwork. No credentials copied.`);
