import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';
import { torrentSearchPreview } from './scripts/torrent-search-preview.mjs';
import { createReadStream, statSync } from 'node:fs';
import path from 'node:path';
const mobileDemo = process.env.VITE_LUMA_MOBILE_DEMO === 'true';
const mobileBuild = process.env.VITE_LUMA_MOBILE === 'true';
const mobileDemoAssets: Plugin = {
	name: 'luma-mobile-demo-assets', apply: 'serve',
	configureServer(server) {
		if (!mobileDemo) return;
		server.middlewares.use('/mobile-demo', (request, response) => {
			const filename = (request.url ?? '').split('?')[0].replace(/^\//, '');
			if (!/^[\w-]+\.(jpg|png|mp4)$/.test(filename)) { response.writeHead(404).end(); return; }
			const file = path.join(process.cwd(), 'mobile/demo-assets', filename);
			try {
				const size = statSync(file).size;
				response.setHeader('Content-Type', filename.endsWith('.mp4') ? 'video/mp4' : filename.endsWith('.png') ? 'image/png' : 'image/jpeg');
				response.setHeader('Content-Length', size);
				createReadStream(file).pipe(response);
			} catch { response.writeHead(404).end(); }
		});
	}
};

const discoveryDiagnostics: Plugin = {
	name: 'luma-discovery-diagnostics',
	apply: 'serve',
	configureServer(server) {
		server.middlewares.use('/__luma-dev/discovery-error', (request, response) => {
			const url = new URL(request.url ?? '/', 'http://127.0.0.1');
			console.error('Discovery UI:', JSON.stringify(url.searchParams.get('message')?.slice(0, 2000)));
			response.writeHead(204).end();
		});
	}
};

export default defineConfig({
	build: { target: mobileBuild ? 'chrome87' : undefined },
	plugins: [
		mobileDemoAssets,
		discoveryDiagnostics,
		torrentSearchPreview(),
		sveltekit({
			outDir: mobileBuild ? '.artifacts/mobile-svelte-kit' : '.svelte-kit',
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},

			adapter: adapter({ fallback: 'index.html', ...(mobileBuild ? {pages: '.artifacts/mobile-web', assets: '.artifacts/mobile-web'} : {}) })
		})
	],
	server: {
		host: '127.0.0.1',
		port: 1420,
		strictPort: true,
		proxy: {
			'/__media-server': {
				target: 'http://127.0.0.1:8940',
				changeOrigin: true,
				rewrite: (path) => path.replace('/__media-server', '/api'),
				configure: (proxy) => proxy.on('proxyReq', (request) => {
					request.setHeader('Origin', 'http://127.0.0.1:8940');
				})
			}
		},
		watch: {
			ignored: ['**/.artifacts/**', '**/src-tauri/**', '**/mobile/src-tauri/**', '**/native/libtorrent-bridge/build/**', '**/media-server/.runtime/**', '**/media-server/tools/**']
		}
	}
});
