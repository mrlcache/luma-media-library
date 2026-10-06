import {invoke} from '@tauri-apps/api/core';
import {nativeMobile} from './mobile-connection';
import {PlayerPresentationQueue} from './player-presentation-queue';
const presentationQueue = new PlayerPresentationQueue(
    (value) => invoke('mobile_player_presentation', value),
    (error) => console.warn('Android player presentation unavailable', error)
);
let presentationOwner: symbol | null = null;
export function beginMobilePlayerPresentation() {
    const owner = Symbol('Mobile player presentation');
    presentationOwner = owner;
    document.documentElement.dataset.mobilePlayerActive = 'true';
    const native = nativeMobile && '__TAURI_INTERNALS__' in window;
    return {
        setControlsVisible(visible: boolean) {
            if (presentationOwner === owner && native) presentationQueue.update({active:true, controlsVisible:visible});
        },
        dispose() {
            if (presentationOwner !== owner) return;
            presentationOwner = null;
            delete document.documentElement.dataset.mobilePlayerActive;
            if (native) presentationQueue.update({active:false, controlsVisible:false});
        }
    };
}
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
