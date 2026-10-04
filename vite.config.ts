import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';
import { torrentSearchPreview } from './scripts/torrent-search-preview.mjs';

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
	plugins: [
		discoveryDiagnostics,
		torrentSearchPreview(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},

			adapter: adapter({ fallback: 'index.html' })
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
			ignored: ['**/.artifacts/**', '**/src-tauri/**', '**/native/libtorrent-bridge/build/**', '**/media-server/.runtime/**', '**/media-server/tools/**']
		}
	}
});
