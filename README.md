# Unnamed Media Platform

Windows-first desktop media library in progress. The product name is intentionally unset; all visible application copy is English.

## Current phase

Desktop foundation: reuse the existing Svelte 5/SvelteKit UI in a Tauri 2/WebView2 shell. The catalog and Player are still fixture-backed visual prototypes.

The current Home and shared shell are the canonical visual reference. Read these files before changing UI:

1. `DESIGN_SYSTEM.md`
2. `PROJECT_STATE.md`
3. `DECISIONS.md`
4. `LUNA_HANDOFF.md`

The broader technical plan and autonomous execution protocol live in:

```text
C:\Users\muris\OneDrive\Imagens\Documentos\ChatGPT\Bountys
```

The first local media slice is in progress: desktop Settings can select a folder and index recognized video files into a local SQLite database. The Rust core now keeps stable file identities and serves paginated catalog records through a narrow Tauri command. The visual catalog is still fixture-backed. Codec probing, playback and UPnP/DLNA sharing are not implemented yet. The browser preview cannot select local folders. ADR-013 records UPnP/DLNA as the primary LAN delivery target, with automatic transcoding when needed.

## Development

```sh
npm install
npm run desktop:dev
```

Validation:

```sh
npm run check
npm run build
npm run desktop:build
```

`npm run build` creates the local static frontend in `build/`; the desktop command also compiles Rust and packages the Windows application. The browser preview remains available with `npm run dev` at `http://127.0.0.1:1420/`. See `PROJECT_STATE.md` for the native-build limitation on this machine.

Do not add product branding, a general-purpose UI kit, eager player loading or large scrolling live-blur surfaces without an accepted architectural decision.
