# Project State

Current phase: Phase 1 — Browser visual prototype  
Current milestone: Canonical Home v2 and shared material shell  
Status: Implemented and under visual review; not user-approved yet  
Repository state: Git repository initialized on `master`; initial implementation snapshot committed

## Current implementation

- SvelteKit/Svelte 5 prototype is running at `http://127.0.0.1:5174/`.
- Desktop uses an inset translucent application shell with a 224px lighter material sidebar and darker cinematic workspace.
- Mobile uses an artwork-overlay top bar and a separate floating material bottom navigation.
- Home keeps Continue Watching in the first desktop fold and uses dedicated desktop/mobile hero compositions.
- Locally bundled Manrope variable typography replaces the previous generic system stack.
- Phosphor replaces one-off SVG icon drawings through the shared `Icon.svelte` wrapper.
- Card hover/focus uses restrained Apple TV-like scale, edge and shadow feedback.
- Player code remains behind the existing lazy boundary.

## Performance decisions

- The single local font asset is approximately 25 KiB WOFF2 and uses `font-display: swap`.
- Phosphor icons are imported individually rather than as a whole runtime set.
- Large scrolling content uses a low-resolution pre-blurred artwork derivative; live `backdrop-filter` stays on fixed shell/control surfaces.
- Reduced-transparency and reduced-motion fallbacks remain explicit.

## Visual authority

- `DESIGN_SYSTEM.md` v2 and the rendered Home are binding for future routes.
- ADR-005 supersedes the rejected horizontal-shell direction.
- Visual acceptance belongs to the user; the current state remains provisional.

## Next unblocked milestone

After Home approval, translate the same shell and component grammar to Library/Search, Details, Settings and Player one surface family at a time.

## Reserved decisions

- Product name and branding remain unset.
- Framework comparison, backend scope, codec/device matrix and desktop/mobile native gates remain governed by the second-brain architecture documents.

## Latest validation

- Rendered inspection completed at the normal desktop viewport and 390×844.
- `npm run check`: 0 errors and 0 warnings.
- `npm run build`: successful.
- Adapter selection remains intentionally unset for the browser prototype; `adapter-auto` reports that no production host has been selected.
