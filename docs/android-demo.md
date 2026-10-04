# Android builds

## Connected mobile build

Run `powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-android-demo.ps1 -Real` to generate `Downloads/Luma-Mobile.apk`. This build uses real backends and does not include the demo artwork, simulated torrents or test video. It targets ARM32 and Android 8+ for devices such as the Galaxy A02s. The existing test package identifier is retained so it can update the previously installed APK.

Enable **Mobile connection** in desktop Settings. On first launch, the phone discovers Luma on the same LAN. Select the computer and approve the matching six-digit code on the PC. The phone stores its connection in Android app-private storage and reconnects using that saved authorization. If broadcast discovery is blocked by a router, enter the computer address displayed in desktop Settings. Desktop TCP/UDP port is 47631; Windows must allow Luma on the private network.

PC downloads use the desktop torrent engine and its registered folders. Phone downloads use a separate librqbit session in app-private Downloads, with persisted transfers, pause/resume, removal and bandwidth limits. They do not become PC files. Completed files appear in the phone library; TMDb metadata can be matched through the paired computer. Incomplete files are excluded from playable media. Android may suspend downloads when the app is backgrounded; a foreground download service remains future work.

Playback streams signed URLs from the PC or reads completed local files. Signed video URLs support byte ranges and sidecar VTT subtitles. Android WebView must support the selected file's codecs; automatic transcoding is not part of this build. Player gestures control app-window brightness and Android media volume. UPnP casting discovers MediaRenderer devices and sends PC media to compatible TVs; Chromecast-specific protocols are not implemented.

The PC bundles a private Node release resolver, started only on demand, so release search does not require Vite. YTS/EZTV use their provider endpoints. 1337x uses the user's configured Prowlarr service; no API keys are shipped in the APK. The DEV configuration is read from `.artifacts/tools/prowlarr/data/config.xml`; an installed desktop uses `<appData>/prowlarr/config.xml`.

The first native build compiles dependencies and takes longer. Subsequent builds reuse Rust/Gradle caches. `scripts/start-android-dev.ps1` supports USB hot reload for frontend work; native Rust/Kotlin changes still require rebuilding. Every APK build first mounts the packaged frontend in an offline DOM test. This verifies startup, but real device, LAN and TV tests are still necessary.

## Offline visual demo

The Android shell lives in `mobile/src-tauri` and shares the Svelte UI with the desktop app. It has its own package ID, `app.luma.mobile.demo`, and is built with `VITE_LUMA_MOBILE=true` and `VITE_LUMA_MOBILE_DEMO=true`.

The demo contains 24 titles, 20 simulated transfers, simulated release results and a generated test video. Images are bundled for offline use. It does not download real torrents or connect to the desktop server. Playback uses an HTML video element with mobile controls and subtitle sheets.

Demo adapters are compiled into the app only when the demo build flag is enabled. They route commands to local fixtures without replacing Tauri's protected native bridge. A normal desktop build does not include them. Do not use this package ID or the demo flag for a production release.

## Build on Windows

Requirements: Node dependencies installed, Rust Android targets, JDK 17, Android SDK platform/build tools 36, NDK 29.0.13846066 and accepted Android SDK licenses.

Run `node scripts/prepare-mobile-demo.mjs` to prepare artwork. This uses the locally saved TMDb read token without embedding it in the app. Demo assets are ignored by Git. Add a generated test video at `mobile/demo-assets/sample.mp4` (the current sample is a 12-second FFmpeg test pattern).

Then run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-android-demo.ps1
```

The default debug APK targets ARM32 for the Galaxy A02s and is copied to the current user's `Downloads/Luma-Mobile-Demo.apk`. Pass `-Target aarch64` for ARM64 or an array of both targets in PowerShell for a universal APK. It is intended for testing on Android 8 or newer. No production signing key is included.

The frontend builds into `.artifacts/mobile-web`, independently of the desktop build and dev server. Its JavaScript targets Chrome 87. An offline DOM smoke test mounts the actual packaged Home before any Android compilation; it checks the read-only native bridge, recommendations, continue watching and handset shell. This catches startup failures quickly but does not replace a real Android WebView test. Run `node scripts/build-mobile-demo.mjs` to build and check just the frontend before creating another APK.

## Hot reload on a USB-connected phone

Enable USB debugging, connect and authorize the phone, then run `scripts/start-android-dev.ps1` in PowerShell. It uses port 1422, forwards that port through ADB and runs the mobile shell in development mode. The initial native build/install still takes time. Subsequent Svelte/CSS changes reload on the phone without generating an APK. Rust/Kotlin changes require a native rebuild. This is separate from the standalone demo APK, which embeds its assets and works offline.

## Mobile 0.2.1 test build

The launcher label is Luma and Android uses the approved desktop icon, including adaptive launcher resources. The APK includes only the architectures requested by the build script, so stale JNI output from an earlier build cannot be packaged accidentally.

The torrent session no longer runs synchronously during application setup. Empty first-launch snapshots do not start the engine; adding a torrent or restoring saved transfers starts it asynchronously. Initialization errors are returned to the download operation instead of aborting the application setup. This removes an identified startup failure path, but the reported device crash still requires a real-device log to confirm its cause.

Mobile cards use smaller TMDb images, native touch scrolling, fewer per-card blur surfaces and no background autoplay iframe. The navigation retains its acrylic treatment, and the explicit Trailer action remains available. Library polling pauses while the app is hidden. No device frame-rate measurement has been performed.
