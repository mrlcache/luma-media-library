import fixtures from './mobile-demo-fixtures.json';
import type { CatalogMedia, TmdbSearchResult } from '$lib/types';

type DemoTitle = CatalogMedia & { tmdbId:number; logoUrl?:string | null };
const titles = fixtures as DemoTitle[];
let installed = titles.slice(0,16);
let transfers = titles.slice(0,20).map((title,index) => ({infoHash:(index+1).toString(16).padStart(40,'0'),name:`${title.title} ${title.kind==='series'?'S01 ':''}1080p`,status:index%4===0?'Paused':'Downloading',error:'',progress:.08+(index%10)*.075,sizeBytes:2_000_000_000,downloadedBytes:160_000_000+(index%10)*150_000_000,uploadedBytes:0,downloadRate:index%4===0?0:2_500_000,uploadRate:128_000,peers:12,seeds:45,queuePosition:index,etaSeconds:500-index*20}));
const discovery = (title:DemoTitle):TmdbSearchResult => ({...title,id:title.tmdbId,kind:title.kind==='series'?'series':'movie',overview:title.overview??''});

export function demoEpisodes(title:string) {
  const item=titles.find(item=>item.title===title);
  return Array.from({length:16},(_,index)=>({id:(item?.id??1)*100+index+1,season:index<8?1:2,episode:index%8+1,title:`Episode ${index%8+1}`,runtime:45,image:item?.backdropUrl??null,summary:'A sample episode for exploring the mobile interface.'}));
}

export function demoReleases(title:string,scope:{type:string;season?:number;episode?:number}) {
  const marker=scope.type==='movie'?'2025':`S${String(scope.season??1).padStart(2,'0')}${scope.type==='episode'?`E${String(scope.episode??1).padStart(2,'0')}`:''}`;
  return ['2160p','1080p','720p','480p'].flatMap((quality,index)=>['QXR','Sample'].map((group,offset)=>({name:`${title} ${marker} ${quality} x265 [${group}]`,source:'Demo',uploader:group,group,sizeBytes:(4-index)*1_000_000_000,size:'',seeds:80-index*15-offset*7,peers:12,infoHash:(100+index*2+offset).toString(16).padStart(40,'0'),url:'',downloadKey:null,quality,codec:'HEVC',fileType:'MKV',qxr:group==='QXR'})));
}

export async function invokeMobileDemo(command:string,args:Record<string,unknown>={}) {
    const data=args as Record<string,any>;
    const title=titles.find(item=>item.id===Number(data.mediaId))??titles.find(item=>item.tmdbId===Number(data.id))??titles[0];
    switch(command) {
      case 'desktop_bootstrap': return {version:'0.1.3-demo',platform:'android',mediaCoreStatus:'library-index-ready',nativeWindowFrame:false};
      case 'get_library_status': return {folders:['Movies','Series'],rootCount:2,fileCount:installed.length,matchedCount:installed.length,unmatchedCount:0,pendingCount:0,lastScanAt:Math.floor(Date.now()/1000),isScanning:false};
      case 'get_catalog_page': {
        let rows=installed.filter(item=>(!data.kind||item.kind===data.kind)&&(!data.query||item.title.toLowerCase().includes(String(data.query).toLowerCase())));
        if(data.sort==='title')rows=[...rows].sort((a,b)=>a.title.localeCompare(b.title));
        return {items:rows.slice(data.offset??0,(data.offset??0)+(data.count??48)),total:rows.length};
      }
      case 'get_local_title_detail': return title?{tmdbId:title.tmdbId,media:title,files:Array.from({length:title.kind==='series'?4:1},(_,index)=>({mediaId:title.id,fileName:`${title.title} ${index+1}.mp4`,path:'/mobile-demo/sample.mp4',season:title.kind==='series'?1:null,episode:title.kind==='series'?index+1:null})),watchedBefore:null}:null;
      case 'get_discovery_feed': return {featured:titles.slice(0,8).map(discovery),sections:[{title:'For you',items:titles.slice(4,16).map(discovery)},{title:'More to explore',items:titles.slice(12).map(discovery)},{title:'Movies for you',items:titles.filter(item=>item.kind==='movie').map(discovery)}],warning:null};
      case 'get_discovery_title': return title?discovery(title):null;
      case 'get_continue_watching': return installed.slice(0,3).map(item=>({...item,positionSeconds:350,durationSeconds:2700,updatedAt:Date.now()/1000}));
      case 'get_playback_history': return installed.slice(0,4).map(item=>({...item,updatedAt:Date.now()/1000}));
      case 'search_tmdb': return titles.filter(item=>item.title.toLowerCase().includes(String(data.query??'').toLowerCase())).map(discovery);
      case 'get_title_logo': case 'get_discovery_logo': return title?.logoUrl??null;
      case 'get_title_trailer': case 'get_discovery_trailer': return null;
      case 'resolve_media_file': return {path:'/mobile-demo/sample.mp4',subtitles:[],resumePositionSeconds:0};
      case 'torrent_snapshot': return {engine:'Demo',downloadDirectory:'Downloads',transfers};
      case 'torrent_set_paused': transfers=transfers.map(item=>item.infoHash===data.infoHash?{...item,status:data.paused?'Paused':'Downloading'}:item); return;
      case 'torrent_remove': transfers=transfers.filter(item=>item.infoHash!==data.infoHash); return;
      case 'torrent_move_queue': {
        const index=transfers.findIndex(item=>item.infoHash===data.infoHash);
        const target=Math.max(0,Math.min(transfers.length-1,index+Number(data.direction)));
        if(index>=0) { const [item]=transfers.splice(index,1);transfers.splice(target,0,item);transfers=transfers.map((item,queuePosition)=>({...item,queuePosition})); } return;
      }
      case 'torrent_add_magnet': case 'torrent_add_file': case 'torrent_add_data': {
        const hash=String(Date.now()).padStart(40,'0');
        const name=command==='torrent_add_magnet'?new URL(String(data.uri)).searchParams.get('dn')||'Sample download':'Sample download';
        transfers=[...transfers,{infoHash:hash,name,status:'Queued',error:'',progress:0,sizeBytes:1_000_000_000,downloadedBytes:0,uploadedBytes:0,downloadRate:0,uploadRate:0,peers:0,seeds:0,queuePosition:transfers.length,etaSeconds:0}]; return hash;
      }
      case 'rescan_library': return {rootCount:2,fileCount:installed.length,matchedCount:installed.length,unmatchedCount:0,pendingCount:0,metadataError:null};
      case 'plugin:dialog|open': return null;
      case 'native_player_status': return null;
      case 'load_remote_artwork': throw new Error('Demo artwork is bundled locally.');
      default: return null;
    }
}
