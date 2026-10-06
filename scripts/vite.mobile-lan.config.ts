import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import { wifiDevCommandBridge } from './wifi-dev-command-bridge';

// Separate development output: do not share the desktop or APK build caches.
export default defineConfig({
  clearScreen: false,
  // This switch exists only for the isolated Wi-Fi DEV APK and is not inherited by normal builds.
  define: { 'import.meta.env.VITE_LUMA_WIFI_DEV': JSON.stringify('false') },
  cacheDir: '.artifacts/mobile-lan/vite-cache',
  plugins: [wifiDevCommandBridge(), sveltekit({
    outDir: '.artifacts/mobile-lan/svelte-kit',
    compilerOptions: {
      runes: ({ filename }) => filename.split(/[/\\]/).includes('node_modules') ? undefined : true
    },
    adapter: adapter({ fallback: 'index.html',
      pages: '.artifacts/mobile-lan/web', assets: '.artifacts/mobile-lan/web' })
  })],
  server: {
    host: process.env.LUMA_LAN_HOST,
    port: 1424,
    strictPort: true,
    hmr: { protocol: 'ws', host: process.env.LUMA_LAN_HOST, clientPort: 1424 },
    fs: { deny: ['.env', '.env.*', '*.{crt,pem}', '**/.git/**'] },
    watch: { ignored: ['**/.artifacts/**', '**/src-tauri/**', '**/mobile/src-tauri/**',
      '**/native/libtorrent-bridge/build/**', '**/media-server/.runtime/**', '**/media-server/tools/**'] }
  }
});
