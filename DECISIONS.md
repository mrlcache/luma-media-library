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
