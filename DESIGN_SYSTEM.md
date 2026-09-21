# Canonical Design System v2 — Unnamed Media Platform

Status: **provisional until explicit user approval; binding for new UI work**  
Visual authority: the rendered Home plus `src/routes/+layout.svelte`, `src/routes/+page.svelte`, `src/app.css`, `MediaRow.svelte` and the Home variant of `PosterCard.svelte`.

## 1. Product character

The product is a serious personal media application: cinematic, quiet, tactile and precise. It combines a native desktop material hierarchy with Apple TV-like media composition. It is not an admin dashboard, a Jellyfin reskin, a generic streaming template or a collection of glass cards.

The order of importance is:

1. media artwork and playback state;
2. clear surface hierarchy;
3. strong, friendly typography;
4. coherent iconography and focus behavior;
5. decoration only when it explains depth or interaction.

All visible product copy is English. The product remains unnamed until the user chooses a name.

## 2. Change authority

- The current Home is the positive reference. Extend it; do not reinterpret “premium,” “minimal” or “Apple-like” from scratch on each route.
- The user alone approves the visual direction. Passing checks or an agent liking its own screenshot is not approval.
- Old routes may contain obsolete visual generations. Never use them as authority over Home.
- A route may introduce a new component, not a new visual language.
- If an existing component visibly conflicts with this document, replace it instead of preserving it for convenience.

## 3. Foundation

### Typography

- Canonical web family: locally bundled variable **Manrope**, with SF Pro/Segoe UI Variable/system fallbacks.
- The bundled Latin variable font is about 25 KiB WOFF2 and uses `font-display: swap`.
- Use rounded, confident weights: body `470–520`, navigation `550–650`, titles `630–710`.
- Hero titles use tight tracking and deliberate line breaks. Do not use thin display type.
- Sentence case is mandatory. No repeated uppercase eyebrows, marketing labels or decorative metadata headings.
- Section titles are compact and heavy enough to remain legible over material surfaces.

### Icons

- Canonical family: **Phosphor** through `Icon.svelte`, imported per icon.
- Active destinations may use `fill`; inactive destinations use `regular`.
- Use `bold` only for small utility glyphs that otherwise lose clarity.
- Do not mix unrelated icon packs or draw one-off SVGs without a documented missing-symbol reason.
- Icon-only controls require accessible names and at least a 34px interaction target.

### Color

- Near-black content plane, not pure black everywhere.
- Sidebar is visibly lighter and more neutral than the content plane.
- Artwork provides nearly all saturated color.
- Blue-grey accent is reserved for focus/progress, not large decorative surfaces.
- No gold luxury styling, neon, gradient orbs, glowing borders or route-specific accent palettes.

### Geometry

- Desktop preview shell: 20px outer radius, 12px environmental inset.
- Sidebar/control radius: 9–12px. Floating mobile navigation: 18px.
- Media artwork radius: about 12px with a fine highlight edge.
- Do not turn every text group or page section into a rounded container.

## 4. Material hierarchy

Material is an architecture, not an effect sprinkled on components.

1. **Environment:** browser uses a low-resolution, pre-blurred artwork derivative; desktop later exposes actual OS material.
2. **Window plane:** dark translucent tint that preserves faint environmental color.
3. **Sidebar:** lighter grey material, stronger blur/saturation and a visible dividing edge.
4. **Top chrome:** dark translucent strip with a quiet highlight and fewer controls.
5. **Local controls:** brighter material only where interaction needs separation.
6. **Media:** crisp and opaque; never blur posters or thumbnails.

Rules:

- Runtime `backdrop-filter` is allowed on fixed shell surfaces and small controls.
- A large scrolling page must use the pre-blurred derivative, not a live full-page blur.
- `prefers-reduced-transparency` must switch to intentional opaque surfaces.
- Browser transparency is an internal simulation; a regular tab cannot reveal the desktop wallpaper through browser chrome.
- Tauri will replace the simulated environment with Windows Mica/Mica Alt and future macOS vibrancy while preserving the same CSS surface roles.

## 5. Shell

### Desktop

- Use a persistent 224px material sidebar.
- Top: functional collection context, never an invented logo or brand.
- Search is integrated into the sidebar.
- Primary destinations: Home, Library, Movies, Series.
- Bottom: Settings and the current profile. No server-ready status, fake analytics, notification bell or admin sections.
- Active navigation is a soft light material with a filled icon; it must not rely on a colored line alone.
- Main workspace is darker than the sidebar and uses a compact 58px top strip.

### Mobile

- Do not compress the desktop sidebar.
- Use an artwork-overlay top utility bar and a floating, blurred bottom destination bar.
- The Home composition is independently tuned for 390px width.
- Touch behavior never depends on hover-only controls.

## 6. Home

### Hero

- Full-bleed media artwork belongs to the canvas, not inside a generic hero card.
- Desktop height: `clamp(410px, 49svh, 530px)` so Continue Watching begins in the first fold.
- Mobile height: `clamp(445px, 61svh, 540px)`.
- Title, essential metadata, one short synopsis and two actions only.
- Primary action is light and solid. Secondary action is a darker local material.
- Stable directional scrims guarantee text contrast.

### Shelves

- Continue Watching uses 16:9 artwork and a thin progress edge.
- Library uses 2:3 posters.
- A shelf should expose a partial next item when space permits.
- Titles and one metadata line sit below artwork.
- Hover/focus may scale about `1.02`, strengthen the edge/shadow and reveal Play.
- Avoid permanent badges, zoom-heavy animation and glass over every card.

## 7. Route translation

- **Library/Search:** bounded or virtualized poster grid, quiet title and inline controls; no toolbar card or filter-status prose.
- **Details:** continue the hero grammar with artwork in the canvas; no full-page rounded panel.
- **Episodes:** compact thumbnail rows with identity, progress and one direct action.
- **Settings:** native grouped lists and separators; never a SaaS control panel.
- **Player:** edge-to-edge video with transient material controls that auto-hide.
- **Empty/error/loading:** preserve the same surface and type system; no oversized generic illustration.

## 8. Interaction, accessibility and performance

- Default motion is `140–200ms`, limited to opacity, transform, color and shadow.
- Respect reduced motion and reduced transparency.
- WCAG 2.2 AA is the baseline; keyboard and focus behavior are mandatory.
- Catalog-only sessions must not load the player engine.
- Do not add a general UI kit or animation framework.
- Posters have explicit dimensions and responsive derivatives.
- Large collections use bounded DOM/virtualization.
- Avoid scroll listeners that force synchronous layout.
- New dependencies require a bundle, maintenance and license reason.

## 9. Automatic rejection signals

Revise before reporting completion if any of these appear:

- flat black planes with no meaningful surface separation;
- sidebar and content using the same tone;
- weak or decorative blur that does not explain hierarchy;
- generic thin system font or mixed icon families;
- dashboard widgets, readiness status, notification decoration or admin language;
- hero placed in a large generic rounded card;
- glass-card soup, pills everywhere or labels explaining obvious UI;
- unrelated stock imagery dominating the identity;
- oversized hero hiding the first useful shelf;
- mobile that is merely compressed desktop;
- large live-blurred scrolling surfaces;
- claiming visual acceptance without explicit user approval.

## 10. Required review loop

For each surface family:

1. Read this file and the canonical Home implementation.
2. Render at the default desktop size and 390×844.
3. Compare sidebar/content separation, font weight, icon consistency, first-fold usefulness and focus behavior.
4. Exercise keyboard navigation, overflow, reduced motion and reduced transparency.
5. Check console, hydration output, typecheck and production build.
6. Record visible weaknesses honestly.
7. Ask for visual judgment only after the implementation is actually open and inspectable.
