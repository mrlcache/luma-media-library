import { invoke as nativeInvoke } from '@tauri-apps/api/core';
import { recoverMobileRead, mobileMediaOrigin } from './mobile-command-recovery';
export const nativeMobile = import.meta.env.VITE_LUMA_MOBILE === 'true';
export type DownloadTarget = 'pc' | 'phone';
let torrentTarget: DownloadTarget = 'pc';
export function getTorrentTarget(): DownloadTarget { return torrentTarget; }
export function setTorrentTarget(target: DownloadTarget) { torrentTarget = target; }
export const localMobileInvoke = <T>(command: string, args: Record<string, unknown> = {}) => nativeInvoke<T>('mobile_local_command', {command,args});
export type MobileConnection = {url:string | null; paired:boolean};
export const wifiDevMobile = import.meta.env.VITE_LUMA_WIFI_DEV === 'true';

async function wifiDevRequest<T>(path:string, init?:RequestInit):Promise<T> {
	const response=await fetch(path,{...init,cache:'no-store'});
	let body:unknown;
	try{body=await response.json();}catch{throw new Error('Luma Desktop returned an invalid response.');}
	if(!response.ok){
		const message=typeof body==='object'&&body!==null&&'error'in body&&typeof body.error==='string'?body.error:`Luma Desktop request failed (${response.status}).`;
		throw new Error(message);
	}
	return body as T;
}

export async function mobileRemoteCommand<T>(command:string,args:Record<string,unknown>={}):Promise<T> {
	const value = await recoverMobileRead(command, () => wifiDevMobile
		? wifiDevRequest<T>('/__luma-wifi-dev/command',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})})
		: nativeInvoke<T>('mobile_remote_command',{command,args}));
	if(command !== 'resolve_media_file') return value;
	const connection = await readMobileConnection().catch(() => null);
	return mobileMediaOrigin(command, value, connection?.url ?? null);
}

export function readComputerLibraryStatus<T>():Promise<T> {
	return mobileRemoteCommand<T>('get_library_status');
}

export const readMobileConnection = () => wifiDevMobile
	? wifiDevRequest<MobileConnection>('/__luma-wifi-dev/status',{method:'POST',headers:{'Content-Type':'application/json'},body:'{}'})
	: nativeInvoke<MobileConnection>('get_mobile_connection');
export const saveMobileConnection = (url:string,token:string) => nativeInvoke<void>('set_mobile_connection',{url,token});
export const testMobileConnection = () => mobileRemoteCommand('desktop_bootstrap');
export const clearMobileConnection = () => nativeInvoke<void>('clear_mobile_connection');
