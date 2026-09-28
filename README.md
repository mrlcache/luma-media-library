# Luma

Luma is an open-source, Windows desktop media library for organizing and watching a personal movie and TV collection. It scans folders you choose, keeps a local catalog, and brings playback, subtitles, metadata, and torrent management into one interface.

## What it does

- Scans local media folders and builds a searchable library with movie and series details.
- Plays video in the built-in MPV engine or opens it with supported external players.
- Loads local subtitles, lets you adjust subtitle appearance, and searches OpenSubtitles.
- Shows artwork and metadata from TMDB and TVmaze.
- Manages torrent downloads through the bundled libtorrent bridge.
- Stores the library and playback history locally on your computer.

Luma does not include a demo library or media files. You select the folders it scans. Metadata and artwork require an internet connection; playback and local library scanning work with your own files.

## Run from source on Windows

Install Node.js 22 or later, Rust stable with the MSVC toolchain, Visual Studio C++ Build Tools, CMake, and vcpkg. The desktop build also needs the WebView2 runtime; the installer can download its bootstrapper.

In PowerShell, set `VCPKG_ROOT` to your vcpkg installation and run:

```powershell
npm ci
npm run check
npm run desktop:dev
```

`desktop:dev` prepares the native playback and torrent engines before starting the live development app. For a production installer, run:

```powershell
npm run desktop:build
```

The installer is written under `src-tauri/target/release/bundle/nsis/`. `npm run build` builds only the frontend; use `npm run desktop:build` to package the Windows app with its native engines.

The torrent bridge uses the `x64-windows-static-md` vcpkg triplet and the pinned baseline in `native/libtorrent-bridge/vcpkg.json`. The preparation script builds the bridge when its source has changed.

## MPV and native binaries

The Windows MPV development archive is pinned in `scripts/prepare-mpv.mjs`. The preparation script downloads the upstream archive only when needed, verifies its SHA-256 checksum, and copies the runtime DLLs into Tauri's ignored `target` directory. No generated engine binaries or personal media are stored in the repository.

The bundled MPV build comes from [mpv-winbuild-cmake](https://github.com/shinchiro/mpv-winbuild-cmake/releases/tag/20260928) and uses [mpv](https://github.com/mpv-player/mpv), licensed under GPL-2.0-or-later. See [Third-party notices](THIRD_PARTY_NOTICES.md) and [LICENSE](LICENSE) for license information.

## Development checks

```powershell
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --workspace
```

The Tauri desktop build and native playback engine target Windows. The Vite frontend can be run on its own with `npm run dev`, but features that use the desktop shell or native engines require Tauri.

## License

Luma is available under the GNU General Public License, version 2 or (at your option) any later version. See [LICENSE](LICENSE). Third-party components retain their own licenses as described in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
