# Developing Luma

[← Back to Luma](../README.md)

## Windows prerequisites

- Node.js 22 or later.
- Rust stable with the MSVC toolchain.
- Visual Studio C++ Build Tools, CMake and vcpkg.
- WebView2 runtime. The Windows installer can download its bootstrapper.

Set `VCPKG_ROOT` to your vcpkg installation before preparing the native engines.

## Live development

```powershell
npm ci
npm run check
npm run desktop:dev
```

`desktop:dev` prepares MPV and the torrent engine, then starts the Tauri app with the Vite development server. Frontend changes update live; Rust changes still require native recompilation.

`npm run dev` starts only the browser frontend. Features that use the desktop shell or native engines require Tauri.

## Build the Windows installer

```powershell
npm run desktop:build
```

The NSIS installer is written to `src-tauri/target/release/bundle/nsis/`. Opening the resulting `.exe` starts the installation wizard.

`npm run build` builds only the frontend. Use `desktop:build` to package the desktop application with its native engines.

On a machine with limited available memory, run the build sequentially with:

```powershell
$env:NODE_OPTIONS = '--max-old-space-size=256'
$env:RAYON_NUM_THREADS = '1'
$env:CARGO_BUILD_JOBS = '1'
npm run desktop:build
```

## Checks

```powershell
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --workspace
```

Verify the installed application separately by opening its Windows Start Menu shortcut. Confirm the version in Settings, library artwork, native playback and subtitles.

## Native dependencies

The MPV archive is pinned in [`prepare-mpv.mjs`](../scripts/prepare-mpv.mjs). The script downloads it when needed, verifies its SHA-256 checksum, and copies the runtime DLLs into Tauri's ignored `target` directory. The checksum is streamed to avoid loading the entire archive into memory.

The bundled build comes from [mpv-winbuild-cmake](https://github.com/shinchiro/mpv-winbuild-cmake/releases/tag/20260928) and uses [MPV](https://github.com/mpv-player/mpv), licensed under GPL-2.0-or-later.

The torrent bridge uses the `x64-windows-static-md` vcpkg triplet and the pinned baseline in [`vcpkg.json`](../native/libtorrent-bridge/vcpkg.json). [`prepare-windows.mjs`](../scripts/prepare-windows.mjs) builds it when its source has changed and the toolchain is available.

Generated engine binaries belong in build output, and personal media, credentials and catalog backups must stay out of Git. See [LICENSE](../LICENSE) and [third-party notices](../THIRD_PARTY_NOTICES.md).
