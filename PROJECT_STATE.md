# Project State

Updated: 2026-09-24
Current phase: Windows-first desktop implementation
Current milestone: Validate local catalog, details and playback end to end; then implement UPnP AV/DLNA

Latest implementation checkpoint: `53f4b16` (`Add local media playback and catalog features`)

Branch: `codex/desktop-shell`

## Current state

The Windows desktop app keeps the existing Svelte 5/SvelteKit UI inside Tauri 2 and WebView2. The installed frontend is bundled locally, so opening the shell does not require a hosted site or internet access. Local catalog and playback flows are intended to work offline. TMDb metadata/artwork, YouTube trailers and OpenSubtitles lookup require a connection when used.

The local SQLite catalog is connected to the desktop UI. TMDb metadata, posters, Home hero PNG logos with title-text fallback, trailer lookup, title details, episode artwork selection, local player controls and subtitle integrations are present in the implementation. Local playback and subtitle flows still need full end-to-end validation on the target PC. The selected UPnP AV/DLNA delivery path and supervised transcoding are not operational yet; LAN sharing remains opt-in and must use the same local library. Tailscale and automatic router port mapping are not prerequisites.

Latest validation: `npm run check` reports 0 errors and two existing `corner-shape` warnings in Details; `npm run build` succeeds; the focused Rust PNG-logo selection test passes. A separate preview window showed a TMDb PNG logo in the Home hero. The final 17 px spacing change was committed after that visual check and has not been visually rechecked.

The Home hero's vertical spacing is now 17 px between title/logo, metadata, synopsis and actions on desktop and narrow layouts. Existing design and UI decisions remain in `DESIGN_SYSTEM.md` and `DECISIONS.md`.

## Goals

- Before product completion, intercept reload shortcuts such as F5, Ctrl+R and Ctrl+Shift+R in the installed app.
- Normal use must not require manual refresh. Show contextual retry only for recoverable errors; handle a fully stalled WebView through the native Tauri shell.

## Next milestones

1. Validate catalog, title details and local playback end to end on the target PC.
2. Implement opt-in UPnP AV/DLNA delivery from the existing local library.
3. Add supervised transcoding only when a receiving device cannot use the original media.
4. Complete the reload-shortcut and recovery goals before calling the desktop app finished.

The older dated sections below are historical implementation records; this summary is the current state. The matching second-brain page is `C:\Users\muris\OneDrive\Imagens\Documentos\ChatGPT\Bountys\PROJECT_STATE.md`.

## Browser prototype record (2026-09-20)

Current phase: Phase 1 — Browser visual prototype  
Current milestone: Phase 1 visual surface family pass
Status: Implemented and validated; stopped at the reserved visual review gate
Repository state: Git repository initialized on `master`; initial implementation snapshot committed

## Current implementation

- SvelteKit/Svelte 5 prototype is running at `http://127.0.0.1:5174/`.
- Desktop uses an inset translucent application shell with a 224px lighter material sidebar and darker cinematic workspace.
- Mobile uses the full workspace canvas and a separate floating material bottom navigation; profile and location remain in the sidebar model rather than a duplicate top bar.
- Home keeps Continue Watching in the first desktop fold and uses dedicated desktop/mobile hero compositions.
- Locally bundled Manrope variable typography replaces the previous generic system stack.
- Phosphor replaces one-off SVG icon drawings through the shared `Icon.svelte` wrapper.
- Card hover/focus uses restrained Apple TV-like scale, edge and shadow feedback.
- Library/Search now share a quiet title, inline search/filter/sort controls, unbadged catalog posters and a bounded virtualized grid.
- Details/Episodes now use a full-canvas backdrop hero, compact episode rows, scoped catalog posters and a dedicated mobile composition.
- Settings now use native grouped lists and separators for Playback, Appearance and the local library surface; the existing toggles and scan action remain functional.
- Player remains behind the existing lazy boundary and now opens as an edge-to-edge surface with transient material controls, keyboard close/play behavior and a dedicated mobile composition.

## Performance decisions

- The single local font asset is approximately 25 KiB WOFF2 and uses `font-display: swap`.
- Phosphor icons are imported individually rather than as a whole runtime set.
- Large scrolling content uses a low-resolution pre-blurred artwork derivative; live `backdrop-filter` stays on fixed shell/control surfaces.
- Reduced-transparency and reduced-motion fallbacks remain explicit.

## Visual authority

- `DESIGN_SYSTEM.md` v2 and the rendered Home are binding for future routes.
- ADR-005 supersedes the rejected horizontal-shell direction.
- Visual acceptance belongs to the user; the current state remains provisional.

## Next decision gate

User visual review of the complete Phase 1 surface family: Home/sidebar, Library/Search, Details/Episodes, Settings and Player. The implementation remains provisional; no user approval is being inferred.

## Reserved decisions

- Product name and branding remain unset.
- Framework comparison, backend scope, codec/device matrix and desktop/mobile native gates remain governed by the second-brain architecture documents.

## Latest validation

- Rendered inspection completed for Home, Library/Search and Details/Episodes at the normal desktop viewport and 390×844.
- Library/Search captures and their before-state are in `.artifacts/ui-captures-library-search`.
- Search, filter, empty/reset, mobile overflow, reduced-motion and catalog-only player loading were exercised in the running browser with no console or HTTP errors.
- Details/Episodes captures are in `.artifacts/ui-captures-details`; bookmark, season selection, episode playback/open-close, mobile overflow and lazy player behavior were exercised with no console or HTTP errors.
- Settings captures are in `.artifacts/ui-captures-settings`; grouped-list structure, toggle state changes, scan action presence, mobile overflow and reduced-motion behavior were exercised with no console errors.
- Player captures are in `.artifacts/ui-captures-player`; Home and Details entry points, lazy-load boundary, edge-to-edge stage geometry, play/pause, Escape close, mobile overflow and reduced-motion behavior were exercised with no console errors.
- `npm run check`: 0 errors and 0 warnings.
- `npm run build`: successful.
- Adapter selection remains intentionally unset for the browser prototype; `adapter-auto` reports that no production host has been selected.

## 2026-09-22 desktop continuation

- Local folder selection, bounded file scanning, SQLite persistence and truthful Settings status were added without changing the visual layout; the indexer is an isolated Rust crate.
- The scan uses staged generations and publishes only when traversal completes; symlinks are not traversed and root depth/file-count limits are enforced.
- `npm run check`: 0 errors, 0 warnings. `npm run build`: successful. `cargo check -p media-core -j 1`: successful. `cargo fmt --all -- --check`: successful after formatting.
- An earlier `cargo check -j 1` exhausted memory in upstream `tauri-utils`. A later `npm run desktop:dev` with one compiler job completed and opened the native window after memory headroom improved. The Rust commands compiled, but the folder dialog and scan have not been exercised end to end.
- Tauri logged a drag permission error on first launch; `core:window:allow-start-dragging` was added to the main-window capability and the app rebuilt/relaunched. Confirm dragging and visual parity in the live window.

## 2026-09-23 HSS Desktop Acrylic candidate

- Replaced the extra native overlay HWND and undocumented Windows 10 fallback with `DWMWA_SYSTEMBACKDROP_TYPE` on the existing Tauri HWND. HSS visibility toggles `DWMSBT_TRANSIENTWINDOW` (Desktop Acrylic on Windows 11 22H2+); unmount/document-visibility/reduced-transparency disables it. The bridge sends no bounds or resize/move IPC. Home only marks native success/failure and supplies an opaque fallback; the live window/server were not restarted or foregrounded.
- This is window-scoped DWM Acrylic, not a true element-scoped backdrop; transparent WebView2 regions reveal it, while opaque areas (notably the sidebar and hero artwork) mask it. Blur intensity is controlled by Windows. The host is Windows 11 build 26200.
- `cargo fmt --all -- --check`, `cargo check -p media-platform-desktop -j 1`, `npm run check` (0 errors/warnings) and a serial `npm run build` pass. A full native binary build was attempted in parallel with Vite and both hit memory exhaustion; avoid parallel builds and defer another full build until the machine is idle.
- Not visually/runtime validated yet. When the user is available, inspect only the existing app: verify material is confined to HSS, the sidebar stays opaque, and moving/resizing/maximizing/focus changes cause no glitches. Do not restart or foreground it while the user is using the computer. Keep ADR-012 provisional until then.

## 2026-09-23 Acrylic composition investigation

- Read the resolved Tao 0.35.3 Windows implementation: setting the Tauri window `transparent: true` also invokes legacy `DwmEnableBlurBehindWindow` on window creation; our Acrylic bridge later sets `DWMWA_SYSTEMBACKDROP_TYPE`. Microsoft's docs say the legacy call no longer produces blur on Windows 8+, while also noting that window alpha is honored, so this is not evidence of an Acrylic conflict and the glitch cause remains unknown. Microsoft documents `DWMWA_REDIRECTIONBITMAP_ALPHA` (Windows 11 build 26100+) to honor alpha in a window redirection bitmap; it defaults off and requires premultiplied alpha. This is only a candidate for an isolated WRY test, not a verified WebView2 fix. Do not change production behavior without visual evidence.
- Lightweight source review only (no app launch or build): desktop `html`, `body`, app stage/shell/workspace and Home page are transparent; the sidebar uses an opaque gradient; HSS (`.home-content`) has translucent gradients but no CSS backdrop filter. Thus the intended blur must come from DWM, with opaque UI masking the HWND-wide material. This confirms the source composition, not runtime appearance or stability.
- Additional official-API path identified, not implemented: WinUI 3's XAML WebView2 control cannot have a transparent background, but the lower-level Win32 WebView2 API supports a fully transparent background and `CoreWebView2CompositionController` can connect to a Composition visual tree. Windows separately documents a non-UWP host-backdrop composition brush and effect route (`DWMWA_USE_HOSTBACKDROPBRUSH` + `CreateHostBackdropBrush`). Combining these to produce a clipped, stronger HSS-only blur appears feasible enough for an isolated POC, but is not yet proven. It would replace the WRY controller/host integration and needs input/focus/accessibility/resize handling; do not migrate production before visual and memory evidence.
- Registry-source check (read-only): locked WRY 0.55.1 already requests transparent WebView2 pixels with RGBA alpha 0 and uses `ICoreWebView2Controller`; it does not use `ICoreWebView2CompositionController`. The locked `webview2-com-sys` 0.38.2 crate contains generated CompositionController/root-visual APIs, so a later Rust POC can likely use the existing COM bindings instead of introducing another WebView2 binding stack. No compile was run.
- Tauri 2.11.6's Windows `with_webview` extension returns the normal WebView2 controller and environment, not a CompositionController. Therefore composition rendering is a host/runtime change from initial WebView creation, not something the current acrylic command can toggle. Preserve the existing single browser instance when prototyping; a hidden second WebView would add memory and is not an acceptable shortcut.
- Visual-strength implication: the current desktop HSS disables CSS `backdrop-filter`; `DWMWA_SYSTEMBACKDROP_TYPE` selects a Windows material kind but exposes no blur-radius knob. CSS radius changes cannot strengthen DWM Desktop Acrylic. A custom host-backdrop Composition blur could expose a radius, but its GPU/memory cost is unmeasured and is a separate POC, not a CSS tweak.
- Added `src-tauri/native-backdrop-poc/app`, an isolated WinUI 3 / Windows App SDK 2.5.1 sample with opaque sidebar/hero and one bounded HSS `SystemBackdropElement`, behind XAML content on one half and transparent WebView2 on the other. .NET SDK 10.0.401 was installed side-by-side; the current single-element revision built successfully with one MSBuild worker and launched after explicit user approval. The user's screenshot is inconclusive for Acrylic visibility; WinUI 3 XAML WebView2 transparency remains unsupported, so that half is diagnostic only and this is not a drop-in Svelte host.
- Added `src-tauri/native-backdrop-poc/wry`, a launch-guarded DWM baseline comparing transparent and opaque top-level windows with one transparent WRY WebView2. HSS tint is reduced to roughly 30–38% to expose the native material; native window chrome and in-page click/keyboard/focus/resize indicators are included for later manual validation. This is not a DirectComposition-hosting test. Its Rust check was stopped by virtual-memory exhaustion in the generated `windows` crate; the current revision is unbuilt and unlaunched. Do not run visual checks or builds while the user reports memory pressure or is using the computer.
- Composition interop review: Microsoft's pure Win32 `WebView2SampleWinComp` source creates a single CompositionController, queries `ICoreWebView2Controller` from that same COM object, attaches a Windows.UI.Composition visual, updates bounds on resize and forwards mouse messages. It demonstrates compatible WebView visual hosting, but not a host-backdrop blur. The WinUI 3 issue is separate: WinUI's `Microsoft.UI.Composition.Visual` has no documented bridge in WebView2 feedback #3439; that risk does not block the pure Win32 sample route. If DWM fails visually, adapt this official sample in isolation. No build/test/launch while memory pressure persists.
- A lower-disruption candidate surfaced in official WRY source review: open PR [tauri-apps/wry #1762](https://github.com/tauri-apps/wry/pull/1762) adds opt-in DirectComposition visual hosting, one controller/WebView, host input/focus/cursor/bounds/move handling, and a per-HWND registration API specifically for internal embedders such as `tauri-runtime-wry`. The PR remains open/unreviewed; its Windows test and Clippy CI jobs succeeded, but composition-specific visual runtime was not tested by CI. The author reports manual UAT in a production fork; this repo locks WRY 0.55.1 without the feature. Tauri 2.11 creates config windows before `.setup`, so registration there would be too late; `Window::add_child` supports the bare-window/pre-register/attach sequence only behind Tauri's `unstable` feature. This may preserve the Tauri/Svelte architecture, but needs upstream/lifecycle verification and a single-view POC. No build/test/launch while memory pressure persists.
- The PR's pending-target registry is thread-local inside WRY. Any app-side registration must use the same globally patched WRY package instance that `tauri-runtime-wry` uses, on the UI thread; a second WRY crate source/version would not share the registry. Verify Cargo resolution before testing this route.
- Verification freshness: only the isolated WinUI POC was built in this session (`dotnet build --no-restore --disable-build-servers -m:1 -p:UseSharedCompilation=false -p:BuildInParallel=false`; 0 warnings/errors). It was launched with the explicit visual-test argument. The main Tauri app and WRY baseline have not been rebuilt; the WRY Cargo check previously exhausted virtual memory. HSS CSS/product UI and the dev server were left untouched.
- Memory/dependency audit: the unused `@fontsource-variable/manrope` npm dependency was removed; the app continues to serve its existing `static/fonts/manrope-latin-variable.woff2` asset. This reduces package/disk footprint, not idle RAM. `node_modules`, `.svelte-kit`, and `build` were retained because the dev server is running and deleting them would not release RAM. System-wide .NET 6 and x86 .NET 8 SDKs were not removed: they are not loaded while idle, may serve other projects, and uninstalling them would reclaim disk rather than working memory.
