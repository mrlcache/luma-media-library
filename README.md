# Unnamed Media Platform

Browser-first prototype for a self-hosted media library and player. The product name is intentionally unset; all visible application copy is English.

## Current phase

Phase 1: visual and interaction prototype in Svelte 5/SvelteKit.

The current Home and shared shell are the canonical visual reference. Read these files before changing UI:

1. `DESIGN_SYSTEM.md`
2. `PROJECT_STATE.md`
3. `DECISIONS.md`
4. `LUNA_HANDOFF.md`

The broader technical plan and autonomous execution protocol live in:

```text
C:\Users\muris\OneDrive\Imagens\Documentos\ChatGPT\Bountys
```

## Development

```sh
npm install
npm run dev
```

Validation:

```sh
npm run check
npm run build
```

Do not add product branding, a general-purpose UI kit, eager player loading or large scrolling live-blur surfaces without an accepted architectural decision.
