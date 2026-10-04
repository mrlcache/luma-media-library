import {invoke} from '$lib/platform/invoke';
import {discoveryMedia} from './discovery';
import type {MediaItem,TmdbSearchResult} from '$lib/types';
type Section={title:string;items:MediaItem[]};
let cached:{time:number;sections:Section[]}|null=null;
export async function readGenreBrowse():Promise<Section[]> {
	if(cached && Date.now()-cached.time<15*60_000)return cached.sections;
	const data=await invoke<{title:string;items:TmdbSearchResult[]}[]>('get_search_browse');
	const sections=data.map(section=>({...section,items:section.items.map(discoveryMedia)}));
	cached={time:Date.now(),sections};
	return sections;
}
