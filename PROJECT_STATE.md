# Project State

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
