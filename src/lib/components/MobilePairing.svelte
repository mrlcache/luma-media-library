<script lang="ts">
	import {onMount} from 'svelte';
	import {invoke} from '@tauri-apps/api/core';
	import {nativeMobile,readMobileConnection} from '$lib/platform/mobile-connection';
	import {isDesktopRuntime,invalidateCatalogPageCache} from '$lib/platform/desktop';
	import {resetDiscovery} from '$lib/media/discovery';
	import Icon from './Icon.svelte';
	let {preview=false}=$props<{preview?:boolean}>();
	type Request={id:string;name:string;code:string};
	let visible=$state(preview), busy=$state(false), message=$state(''), code=$state(''), address=$state('');
	let computers=$state<{name:string;url:string}[]>([]), requests=$state<Request[]>([]);
	let poll:ReturnType<typeof setInterval>|undefined;
	let disposed=false;
	async function discover(){
		busy=true;message='';
		try{computers=await invoke('discover_luma_computers');if(!computers.length)message='Open Luma on your computer and enable Mobile connection in Settings.';}
		catch(error){message=String(error);}finally{busy=false;}
	}
	async function connect(url:string){
		if(poll)clearInterval(poll);
		busy=true;message='';
		try{
			const request=await invoke<Request>('request_mobile_pair',{url});
			code=request.code;
			let checking=false;
			poll=setInterval(async()=>{
				if(checking||disposed)return;checking=true;
				try{
					if(await invoke<boolean>('check_mobile_pair',{url,id:request.id})){
						if(poll)clearInterval(poll);code='';visible=false;
						invalidateCatalogPageCache();resetDiscovery();
						window.dispatchEvent(new Event('luma-library-changed'));
					}
				}catch(error){if(poll)clearInterval(poll);code='';message=String(error);}
				finally{checking=false;}
			},2000);
		}catch(error){message=String(error);}finally{busy=false;}
	}
	async function approve(id:string,allow:boolean){
		try{await invoke('approve_mobile_pair',{id,allow});requests=requests.filter(r=>r.id!==id);}
		catch(error){message=String(error);}
	}
	onMount(()=>{
		if(preview||!isDesktopRuntime()||import.meta.env.VITE_LUMA_MOBILE_DEMO==='true')return;
		const show=()=>{visible=true;code='';void discover();};
		window.addEventListener('luma-pair-computer',show);
		if(nativeMobile){
			void readMobileConnection().then(connection=>{
				if(!connection?.paired && sessionStorage.getItem('luma.phone-only')!=='1')show();
			}).catch(error=>message=String(error));
		}else{
			let checking=false;
			poll=setInterval(async()=>{
				if(checking)return;checking=true;
				try{requests=await invoke('get_mobile_pair_requests');}catch{/* Older desktop builds have no pairing listener. */}
				finally{checking=false;}
			},2500);
		}
		return ()=>{disposed=true;if(poll)clearInterval(poll);window.removeEventListener('luma-pair-computer',show);};
	});
</script>

{#if (nativeMobile || preview) && visible}
	<div class="pair-screen">
		<div class="pair-content">
			<img class="pair-logo" src="/luma-wordmark.svg" alt="Luma" width="120" height="44"/>
			<Icon name="computer" size={34}/>
			<h1>Connect your computer</h1><p>Access your library, stream your media and manage your downloads.</p>
			{#if code}<div class="pair-code">{code}</div><p>Approve this code in Luma on your computer.</p>
			{:else}
				{#each computers as computer}<button type="button" disabled={busy} onclick={()=>connect(computer.url)}><Icon name="computer" size={18}/>{computer.name}<Icon name="chevron-right" size={15}/></button>{/each}
				<button type="button" disabled={busy} onclick={discover}>{busy?'Looking for computers…':'Find computers'}</button>
				<label>Computer address<input bind:value={address} placeholder="http://192.168.1.10:47631" type="url" /></label>
				{#if address.trim()}<button type="button" disabled={busy} onclick={()=>connect(address)}>Connect</button>{/if}
			{/if}
			{#if message}<p role="status">{message}</p>{/if}
			<button class="pair-secondary" type="button" onclick={()=>{visible=false;if(poll)clearInterval(poll);sessionStorage.setItem('luma.phone-only','1');}}>Use this phone only</button>
		</div>
	</div>
{:else if !nativeMobile && requests.length}
	<div class="pair-overlay"><div class="pair-content pair-desktop" role="dialog" aria-label="Approve mobile connection">
		<h2>Connect a phone</h2>
		{#each requests as request}<p>{request.name}</p><div class="pair-code">{request.code}</div><p>Match this code with the one shown on your phone.</p>
		<div class="pair-actions"><button type="button" onclick={()=>approve(request.id,false)}>Decline</button><button type="button" onclick={()=>approve(request.id,true)}>Connect</button></div>{/each}
		{#if message}<p>{message}</p>{/if}
	</div></div>
{/if}

<style>
	.pair-screen,.pair-overlay{position:fixed;inset:0;z-index:300;display:grid;place-items:center;padding:24px;overflow:auto;color:var(--text-soft);background:rgba(10,16,22,.98);}
	.pair-overlay{background:rgba(4,8,12,.6);backdrop-filter:blur(15px);}
	.pair-content{display:grid;justify-items:center;gap:16px;width:min(100%,380px);text-align:center;}
	.pair-desktop{padding:30px;border:1px solid var(--line-strong);border-radius:20px;background:var(--surface-1);}
	.pair-logo{width:120px;height:44px;object-fit:contain;margin-bottom:22px;}
	h1,h2{margin:0;color:var(--text-strong);font-size:1.2rem;}p{margin:0;font-size:.8rem;line-height:1.6;color:var(--text-muted);}
	button{display:flex;align-items:center;justify-content:center;gap:12px;width:100%;min-height:46px;padding:10px 14px;border:1px solid var(--line-strong);border-radius:12px;background:var(--surface-2);color:var(--text-soft);font:inherit;font-size:.85rem;cursor:pointer;}
	button:disabled{opacity:.5;}label{display:grid;gap:8px;width:100%;text-align:left;color:var(--text-muted);font-size:.75rem;}
	input{width:100%;height:46px;padding:0 12px;border:1px solid var(--line-subtle);border-radius:10px;background:var(--surface-1);color:var(--text-soft);font:inherit;}
	.pair-secondary{margin-top:12px;border-color:transparent;background:transparent;color:var(--text-muted);font-size:.75rem;}
	.pair-code{font-size:2rem;font-weight:600;letter-spacing:.22em;color:var(--accent-soft);padding:12px;}
	.pair-actions{display:flex;gap:10px;width:100%;}
</style>
