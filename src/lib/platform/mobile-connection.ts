import { invoke as nativeInvoke } from '@tauri-apps/api/core';
export const nativeMobile = import.meta.env.VITE_LUMA_MOBILE === 'true';
export type DownloadTarget = 'pc' | 'phone';
let torrentTarget: DownloadTarget = 'pc';
export function getTorrentTarget(): DownloadTarget { return torrentTarget; }
export function setTorrentTarget(target: DownloadTarget) { torrentTarget = target; }
export const localMobileInvoke = <T>(command: string, args: Record<string, unknown> = {}) => nativeInvoke<T>('mobile_local_command', {command,args});
export type MobileConnection = {url:string | null; paired:boolean};
export const readMobileConnection = () => nativeInvoke<MobileConnection>('get_mobile_connection');
export const saveMobileConnection = (url:string,token:string) => nativeInvoke<void>('set_mobile_connection',{url,token});
export const testMobileConnection = () => nativeInvoke('mobile_remote_command',{command:'desktop_bootstrap',args:{}});
