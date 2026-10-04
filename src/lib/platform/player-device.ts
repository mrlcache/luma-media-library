import {invoke} from '@tauri-apps/api/core';
import {nativeMobile} from './mobile-connection';
const enabled = () => nativeMobile && import.meta.env.VITE_LUMA_MOBILE_DEMO !== 'true' && '__TAURI_INTERNALS__' in window;
export async function readPlayerLevels():Promise<{volume:number;brightness:number}|null> {
	return enabled() ? invoke('mobile_player_levels') : null;
}
let pending: ReturnType<typeof setTimeout> | undefined;
let next:Record<string,number> = {};
export function setPlayerLevel(level:'volume'|'brightness', value:number) {
	if (!enabled()) return;
	next[level] = value;
	if (pending) return;
	pending=setTimeout(()=>{
		pending=undefined;
		const values=next; next={};
		void invoke('mobile_player_levels',{values}).catch(console.warn);
	},100);
}
export function resetPlayerLevels() {
	if(pending)clearTimeout(pending); pending=undefined; next={};
	if(enabled())void invoke('mobile_player_levels',{values:{reset:true}}).catch(console.warn);
}
