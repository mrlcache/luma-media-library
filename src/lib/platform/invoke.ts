import { invoke as invokeNative, type InvokeArgs, type InvokeOptions } from '@tauri-apps/api/core';
import { invokeMobileDemo } from './mobile-demo';
import { mobileRemoteCommand, nativeMobile } from './mobile-connection';
import type {CatalogMedia,TmdbSearchResult} from '$lib/types';
import { version as applicationVersion } from '../../../package.json';
const metadataAttempts=new Map<number,number>();
let metadataBusy=0;
let lastRemoteContinue:Record<string,unknown>[]|null=null;
function enrichPhoneCatalog(items:CatalogMedia[]) {
	for(const item of items){
		if(item.kind || metadataBusy>=2 || Date.now()-(metadataAttempts.get(item.id) ?? 0)<300_000)continue;
		metadataAttempts.set(item.id,Date.now());metadataBusy++;
		const cleaned=item.title.split(/\s+(?:\(?\d{4}\)?|S\d{1,2}(?:E\d{1,3})?|\d{3,4}p|UHD|BluRay|WEB[- ]?DL|HDR)\b/i)[0].trim();
		const normalize=(value:string)=>value.toLowerCase().replace(/[^\p{L}\p{N}]/gu,'');
		void mobileRemoteCommand<TmdbSearchResult[]>('search_tmdb',{query:cleaned}).then(async results=>{
			const year=/\b(19\d{2}|20\d{2})\b/.exec(item.title)?.[1];
			const matches=results.filter(result=>normalize(result.title)===normalize(cleaned) && (!year||result.year===Number(year)));
			if(matches.length===1)await invokeNative('mobile_local_command',{command:'save_local_metadata',args:{mediaId:item.id,metadata:matches[0]}});
		}).catch(()=>{/* Retry when the computer is available. */}).finally(()=>metadataBusy--);
	}
}

/** Demo commands use local data; native Tauri internals remain untouched. */
export function invoke<T>(command: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
	if (import.meta.env.VITE_LUMA_MOBILE_DEMO === 'true') {
		return invokeMobileDemo(command, args as Record<string, unknown> | undefined) as Promise<T>;
	}
	if (nativeMobile) return invokeMobile<T>(command, (args ?? {}) as Record<string, unknown>);
	return invokeNative<T>(command, args, options);
}

async function invokeMobile<T>(command: string, args: Record<string, unknown>): Promise<T> {
	const remote = <R>(values = args) => mobileRemoteCommand<R>(command, values);
	const local = <R>(values = args) => invokeNative<R>('mobile_local_command', { command, args: values });
	if (command === 'desktop_bootstrap') return {version:applicationVersion, platform:'android', mediaCoreStatus:'library-index-ready', nativeWindowFrame:true} as T;
	if (Number(args.mediaId) >= 1_000_000_000) {
		if (['get_title_logo', 'get_title_trailer'].includes(command)) {
			const detail=await invokeNative<{media:{kind:string},tmdbId?:number}>('mobile_local_command',{command:'get_local_title_detail',args});
			return detail.tmdbId ? mobileRemoteCommand<T>(command==='get_title_logo'?'get_discovery_logo':'get_discovery_trailer',{id:detail.tmdbId,kind:detail.media.kind}) : null as T;
		}
		return local<T>();
	}
	if (command === 'get_catalog_page') {
		type Page = {items:unknown[]; total:number; offset:number};
		const phone = await local<Page>({...args, offset:0, count:200});
		enrichPhoneCatalog(phone.items as CatalogMedia[]);
		let pc:Page;
		try { pc = await remote<Page>(); } catch { return local<T>(); }
		const offset = Number(args.offset ?? 0), count = Number(args.count ?? 48);
		const localOffset = Math.max(0, offset - pc.total);
		const remaining = Math.max(0, count - pc.items.length);
		const extra = remaining ? await local<Page>({...args, offset:localOffset, count:remaining}) : phone;
		return {...pc, items:[...pc.items,...(remaining ? extra.items : [])], total:pc.total+phone.total} as T;
	}
	if (['get_continue_watching','get_playback_history'].includes(command)) {
		const remoteRows = remote<Record<string,unknown>[]>().then(rows=>{
			if(command==='get_continue_watching')lastRemoteContinue=rows;
			return rows;
		}).catch(()=>command==='get_continue_watching'?(lastRemoteContinue ?? []):[]);
		const [pc, phone] = await Promise.all([remoteRows,local<Record<string,unknown>[]>()]);
		return [...pc,...phone].sort((a,b)=>Number(b.updatedAt)-Number(a.updatedAt)).slice(0,Number(args.count ?? 12)) as T;
	}
	if (command === 'get_library_status') {
		const phone = await local<Record<string,unknown>>();
		try {
			const pc = await remote<Record<string,unknown>>();
			return {...pc, folders:[...(pc.folders as string[]),...(phone.folders as string[])], fileCount:Number(pc.fileCount)+Number(phone.fileCount), rootCount:Number(pc.rootCount)+Number(phone.rootCount)} as T;
		} catch { return phone as T; }
	}
	return remote<T>();
}
