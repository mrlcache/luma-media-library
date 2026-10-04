import { isDesktopRuntime } from '$lib/platform/desktop';
import { invoke } from '$lib/platform/invoke';

export async function requestRelease(params:Record<string,string>,signal?:AbortSignal):Promise<any> {
	if(signal?.aborted) throw new DOMException('Cancelled','AbortError');
	if(isDesktopRuntime() && import.meta.env.VITE_LUMA_MOBILE_DEMO !== 'true') {
		const result = await invoke('release_search',{params});
		if(signal?.aborted) throw new DOMException('Cancelled','AbortError');
		return result;
	}
	const response=await fetch(`/__luma-preview/search?${new URLSearchParams(params)}`,{signal});
	const data=await response.json();
	if(!response.ok) throw new Error(data.error || 'Release search could not be completed.');
	return data;
}
