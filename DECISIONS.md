# Decisions

## ADR-001 — Keep product name and brand unset
Date: 2026-09-20
Status: accepted
Context: The source plan explicitly leaves the product name undecided and forbids invented branding during the prototype phase.
Decision: Use no visible product name, logo or invented brand treatment in the UI. The package slug remains an implementation detail only.
Evidence: `MEDIA_PLATFORM_PLAN.md`, `SYSTEM_DESIGN.md` and `LUNA_EXECUTION_PROTOCOL.md` all preserve this gate.
Consequences: Navigation is content-led and the shell must communicate hierarchy without a branded masthead.
Revisit trigger: An explicit user product/brand decision.

## ADR-002 — Reframe Home as a quiet editorial canvas
Date: 2026-09-20
Status: superseded by ADR-004
Context: The first Home implementation read as a generic streaming/dashboard template: a rounded hero card, gold accent, dense shell labels, poster badges and repeated collection links.
Decision: Keep the existing data, routes and interactions but remove decorative containers and labels. Use a neutral multi-surface canvas, system font stack, one restrained blue-grey accent, a borderless content-led hero, clean media rows, unbadged Home posters and a compact shell.
Evidence: The new 1440×900 and 390×844 captures in `.artifacts/ui-captures-redesign` show Continue watching below the hero on desktop and a dedicated mobile composition.
Consequences: Artwork supplies most color; semantic hierarchy comes from type, alignment and whitespace. Shared shell changes affect navigation framing on every route, while Home-only card variants avoid redesigning Library/Details in this milestone.
Revisit trigger: Home review identifies a remaining template/dashboard signal or accessibility/performance evidence requires a different treatment.

## ADR-003 — Hold scope at Home and shell until review
Date: 2026-09-20
Status: accepted
Context: The visual correction request explicitly gates further page work on Home review.
Decision: This execution changes only Home presentation, the shared shell and Home-scoped card/row variants. Library, Details, Settings and Player visuals remain outside the redesign scope.
Evidence: The implementation uses `PosterCard`'s `home` variant and keeps the default card rules for other routes.
Consequences: The prototype may temporarily have different visual generations across routes; consistency work waits for explicit Home approval.
Revisit trigger: Home approval or a newly authorized cross-route visual pass.

## ADR-004 — Use the implemented Home as the visual authority
Date: 2026-09-20
Status: superseded by ADR-005 after explicit user rejection
Context: A prohibition-only redesign removed obvious template elements but produced an empty, generic wireframe. The project needs a positive implementation reference that future work can extend without repeating art-direction decisions.
Decision: The current Home, shared horizontal shell and `DESIGN_SYSTEM.md` form the canonical visual foundation. Future routes reuse their tokens, material budget, composition rhythm and responsive behavior. Agents cannot mark their own visual work accepted.
Evidence: The rendered Home uses full-canvas cinematic artwork, a single restrained chrome layer, content visible in the first fold, horizontal shelves and a dedicated mobile composition. Typecheck and production build pass.
Consequences: Luna can focus on faithful route implementation instead of autonomous art direction. Existing routes remain visually obsolete until migrated one surface family at a time.
Revisit trigger: Explicit user rejection/approval, target-device evidence or a demonstrated accessibility/performance conflict.

## ADR-005 — Adopt a material sidebar shell and bundled visual primitives
Date: 2026-09-20
Status: provisional pending user approval
Context: The horizontal-shell iteration remained visually generic. It lacked the strong light/dark surface separation, perceptible material depth, rounded heavier typography and coherent icon craft requested by the user. The user explicitly authorized replacing poor existing design instead of preserving it.
Decision: Use a 224px lighter material sidebar on desktop, a darker cinematic workspace, a dedicated floating mobile navigation composition, locally bundled Manrope variable typography and per-icon Phosphor imports. Browser mode simulates environmental transparency with a pre-blurred artwork derivative; Tauri later maps the same surface roles to native material.
Evidence: The new rendered Home visibly separates sidebar, chrome, hero and shelf plane at desktop and 390×844. The first useful shelf remains in the initial viewport. Live blur is confined to fixed shell/control layers.
Consequences: ADR-004's horizontal shell is obsolete. Future routes must reuse the v2 shell, font, icon wrapper, material hierarchy and focus behavior. The two small dependencies add a maintained icon set and one approximately 25 KiB WOFF2 font asset; licenses are recorded in `THIRD_PARTY_NOTICES.md`.
Revisit trigger: User visual review, measured low-end GPU cost, accessibility conflict or a native desktop material prototype demonstrating a required structural change.

## ADR-006 — Translate Library/Search as a quiet catalog surface
Date: 2026-09-20
Status: provisional pending user review
Context: Library and Search were still using the pre-v2 dashboard generation: uppercase collection labels, helper prose, a segmented toolbar, filter-status copy and permanent Movie/Series badges on every poster.
Decision: Keep the existing routes, deterministic media data, search/filter/sort behavior and virtualized rendering, but use a quiet title/count header, line-based inline controls, a minimal result line and a catalog PosterCard variant without type badges. Preserve the shared v2 shell, Manrope typography, Phosphor icons and material hierarchy. Let the mobile surface begin on the workspace canvas without compensating for a duplicate top utility bar.
Evidence: Desktop and 390×844 captures in `.artifacts/ui-captures-library-search`; browser assertions confirm 1 search result for “ordinary”, 5 Series results, empty/reset behavior, no horizontal overflow, no catalog badges, no player dialog/chunk on catalog load, and no console/HTTP errors. `npm run check` and `npm run build` pass.
Consequences: Library/Search now belong to the Home visual family without changing Home, Details, Settings or Player styling. `PosterCard` supports scoped `home` and `catalog` variants while the default variant remains available to legacy/detail surfaces. The grid remains bounded and scrollable rather than mounting an unbounded library.
Revisit trigger: User review identifies a visual mismatch, a real-device accessibility/performance measurement exposes a defect, or the future backend contract requires replacing local filtering with server queries.

## ADR-007 — Translate Details/Episodes as a canvas-led content surface
Date: 2026-09-20
Status: provisional pending user review
Context: Details still used a rounded hero panel, uppercase kicker, gold interaction styling, helper note and dense episode rows from the pre-v2 visual generation.
Decision: Preserve route data, bookmark state, season control, episode actions and lazy playback, but move the backdrop into a borderless canvas hero, retain the poster as an editorial anchor, use the Home typography/material grammar, remove redundant helper copy, compress Episodes into quiet rows and use the catalog poster variant for related content. Tune mobile independently on the canvas without a secondary top utility bar.
Evidence: Desktop and 390×844 captures in `.artifacts/ui-captures-details`; browser assertions confirm borderless hero, four episode rows, no related badges, bookmark toggle, season selection, episode player open/close, no mobile overflow and no console/HTTP errors. `npm run check` passes after the implementation.
Consequences: Details belongs to the same visual family as Home and Library/Search without modifying the Player implementation. Episode data still uses the existing fixture for every season until real backend season data exists.
Revisit trigger: User review, a real season-data contract, or measured accessibility/performance evidence requiring a structural change.

## ADR-008 — Translate Settings as native grouped lists
Date: 2026-09-20
Status: provisional pending user review
Context: Settings still used rounded dashboard cards, numbered section labels and generic panel treatment that did not belong to the Home/sidebar material system.
Decision: Preserve the existing Playback, Appearance and local-library settings and their state bindings, but render them as quiet grouped lists with native separators, restrained values, accessible toggles and a small functional scan action. Remove the uppercase workspace kicker, decorative indexes, card containers and gold toggle state.
Evidence: Desktop and 390×844 captures in `.artifacts/ui-captures-settings`; browser assertions confirm three grouped lists, zero legacy setting cards, working autoplay toggle state, no horizontal overflow, reduced-motion fallback and no console errors.
Consequences: Settings now reads as part of the same dark workspace plane as Home, Library/Search and Details rather than as a SaaS control panel. The local server data remains fixture-backed until the backend contract exists.
Revisit trigger: User visual review, a real server/settings contract or accessibility evidence requiring a different grouping.

## ADR-009 — Keep Player edge-to-edge and transient
Date: 2026-09-20
Status: provisional pending user review
Context: The preview player still presented a rounded, blurred stage with uppercase preview language and a generic modal-card silhouette.
Decision: Preserve the lazy `PlayerHost` boundary, existing playback controls, range inputs, Escape close behavior and accessibility labels, but make the player an edge-to-edge viewport surface. Use the artwork as the frame, remove fake readiness copy, keep controls transient and material, and tune mobile independently without changing the app shell.
Evidence: Desktop and 390×844 captures in `.artifacts/ui-captures-player`; browser assertions confirm no player resource before open, resource load after open, zero stage radius/border, play/pause, Escape close, mobile no-overflow and no console errors.
Consequences: Catalog routes remain free of eager player loading, while playback has a stronger cinematic transition from the Home and Details actions. The current frame is still a visual prototype; real media transport remains outside this milestone.
Revisit trigger: User visual review, a real playback/media contract, device performance evidence or accessibility testing of the transient controls.

## ADR-010 — Move to a Windows-first desktop application
Date: 2026-09-22
Status: accepted for implementation; native runtime validation pending
Context: The user explicitly ended the browser-first rollout and authorized translating the existing UI into an application while retaining its current appearance and speed.
Decision: Keep Svelte 5/SvelteKit as a local static SPA inside Tauri 2/WebView2. Use Rust for privileged local media work behind narrow commands. Preserve the current UI and postpone visual redesign. Configure a transparent Windows shell with Mica Dark when available, a CSS fallback, and window-state persistence. Defer the Qwik benchmark and public web deployment.
Evidence: The existing UI passes `npm run check` and static `npm run build`. WebView2, Rust and MSVC are available. Native build was attempted but this host exhausted virtual memory while compiling the upstream `windows` crate, so startup/material/RAM claims are not yet verified.
Consequences: SSR is disabled for the installed frontend and dynamic routes use a static SPA fallback. Media indexing/playback are not yet implemented. This decision supersedes browser-first sequencing in the second-brain plan and Luna handoff.
Revisit trigger: Measured startup, RAM, video compatibility or old-device behavior demonstrates that WebView2/Tauri cannot meet the agreed targets.
