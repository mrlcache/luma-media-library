# Third-party notices

## FFmpeg / ffprobe

- Components: the Windows FFmpeg and ffprobe executables used for HLS transcoding and subtitle inspection.
- License of the bundled build: GPL-3.0; its complete license is included beside the executables as `LICENSE-FFmpeg.txt`.
- Source and build information: [FFmpeg](https://ffmpeg.org/) and the linked Windows build provider [Gyan](https://www.gyan.dev/ffmpeg/builds/).

## hls.js

- Component: mobile HLS playback and buffering.
- Package: `hls.js` 1.7.3.
- License: Apache-2.0; complete notices are distributed in the upstream package.
- Source: [video-dev/hls.js](https://github.com/video-dev/hls.js).

## MPV Windows runtime

- Component: `libmpv-2.dll` and the runtime DLLs from the pinned Windows development archive.
- Upstream package: [mpv-winbuild-cmake 20260928](https://github.com/shinchiro/mpv-winbuild-cmake/releases/tag/20260928).
- Archive: `mpv-dev-x86_64-20260928-git-e470f8986e.7z`.
- SHA-256: `81795d759e01016f1550fd71651a1a5d59ab5c28ef31c0b6793224e9cff39459`.
- The MPV program is licensed under GPL-2.0-or-later. Its upstream source and license information are available at [mpv-player/mpv](https://github.com/mpv-player/mpv). The archive also includes third-party runtime libraries; consult the upstream project and archive for their respective licenses and source.

## Libtorrent

- Component: native torrent engine, built from the vcpkg `libtorrent` port declared in `native/libtorrent-bridge/vcpkg.json`.
- Source: [arvidn/libtorrent](https://github.com/arvidn/libtorrent).
- License: BSD 3-Clause; see the upstream project for the complete text and notices.

## Manrope

- Source package: `@fontsource-variable/manrope`
- Bundled asset: `static/fonts/manrope-latin-variable.woff2`
- License: SIL Open Font License 1.1
- Project: [sharanda/manrope](https://github.com/sharanda/manrope)

## Phosphor Icons

- Source package: `phosphor-svelte`
- License: MIT
- Project: [phosphor-icons/svelte](https://github.com/phosphor-icons/svelte)

## 7zip-bin

- Source package: `7zip-bin` (development dependency used to extract the pinned MPV archive).
- License: MIT
- Project: [7zip-bin](https://github.com/7zip-bin/7zip-bin)

Package distributions in `node_modules` retain their complete license texts. The MPV runtime archive is downloaded during Windows preparation and is not committed to this repository.
