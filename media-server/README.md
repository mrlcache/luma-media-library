# Luma media server prototype

Development prototype integrated into Luma's main DEV session through the Media server tab. Node.js 24+, Windows and an existing Luma library are required. It reads the existing SQLite library without writing to it. It is not yet embedded in the installed app.

## Run

```powershell
# From the main Luma checkout:
npm run desktop:dev:independent
# If FFmpeg has not been set up:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\media-server\setup-ffmpeg.ps1
```

The DEV launcher starts Node independently of Codex without opening another window. Use the Media server switch in Luma to enable/disable media serving and discovery. The switch state persists in `.runtime/server-state.json`. Disabling stops streams, conversion workers, HTTP and SSDP announcements; local controls remain available to enable it again. It does not start automatically at Windows login. A separate diagnostic control panel is available at http://127.0.0.1:8940.

Optional CLI: `node src/main.mjs --host 192.168.1.3 --port 8941 --admin-port 8940 --db "C:\path\media-library.sqlite3" --start`. The host must belong to a local interface. The default is the first active Wi-Fi IPv4 interface. The stable UPnP identity is stored only in `.runtime/identity.json`.

For hot reload, run `npm run dev` and open the control panel in Chrome. Node watches `src/` and `web/` and restarts only this server; the panel reloads when it detects the new version. An edit will interrupt streams from this prototype during restart. It does not restart Luma or the other project's dev server.

## Implemented

- SSDP `M-SEARCH`, alive/byebye multicast, stable MediaServer identity.
- UPnP ContentDirectory Browse, metadata, pagination, title sorting, system updates and event subscriptions; ConnectionManager service.
- Movies and series navigation using Luma's indexed TMDb names, posters and episode positions. Reads SQLite in read-only mode; refreshes every five seconds without rescanning folders.
- Original HTTP GET/HEAD with byte ranges, accurate file size and cancellation. File URLs use item IDs; canonical path checks confine access to the library roots.
- Explicit compatibility resource through FFmpeg, MPEG-TS output: remux H.264/AAC; preserve H.264 while converting incompatible audio; otherwise convert to H.264/AAC. No conversion is applied to the original resource. Maximum two concurrent conversions.
- Compatibility offset through `?start=seconds`; converted streams do not implement byte/time range seeking. Subtitle conversion and HDR tone mapping are not implemented; HDR conversion is rejected rather than silently losing HDR.
- Local control panel and read-only LAN `/api/library` with original/compatibility playback URLs.

## Validation / limits

Run `npm test`. Tests use synthetic files and a temporary database, never the user's database. They cover browse/metadata, range streaming, subscription callbacks, stop/restart, path boundaries and actual FFmpeg output for remux, audio conversion and full conversion.

BubbleUPnP/device discovery and actual playback must still be tested on a second LAN device. Windows Firewall may block inbound TCP 8941 and UDP 1900 depending on existing rules/network profile. No firewall rules or router mappings are changed by this prototype.

Original appears first in DLNA metadata; clients choose among resources. This is not universal automatic client capability negotiation. The compatibility link can be selected explicitly when testing. An eventual Luma client can use device profiles and the LAN API to choose a mode; automatic failure detection is not available from an arbitrary DLNA player.

No remote access, accounts, TLS, discovery across subnets, background Windows service, hardware encoding, or desktop settings integration yet. The media listener is bound to one LAN interface and rejects peers outside its subnet; it has no per-user authentication and is intended only for the home network. UPnP here is media discovery, not router port forwarding.

## References

- [UPnP ContentDirectory specification](https://upnp.org/specs/av/UPnP-av-ContentDirectory-v1-Service.pdf)
- [FFmpeg formats](https://ffmpeg.org/ffmpeg-formats.html)
- [Official FFmpeg download page](https://ffmpeg.org/download.html); Windows executables are downloaded from its linked Gyan provider with SHA-256 verification, kept locally under ignored `tools/`.

Future integration: preserve this protocol/streaming behavior behind a Rust backend managed by Tauri; add controls using the existing Luma settings design. Package FFmpeg and its required licensing materials with the eventual installer. Do not publish personal databases, logs, media files or `tools/` as project source.

## Luma Dev 2 window

`desktop/` is a separate snapshot of the current Luma source with a `Dev 2` badge and a Media server route. Its frontend runs at `127.0.0.1:1422`, leaving the original `1420` development instance untouched. It uses its own Tauri target directory and app identifier. Its private library snapshot is stored in `.runtime/desktop-data`; the UPnP server continues to read the live original library in read-only mode.

Start the media server with `npm run dev` in this workspace, then run `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\dev2.ps1`. Frontend changes update Dev 2 through Vite HMR. The desktop has Rust watch disabled to avoid restarting the window during UI work. The copied compiler cache avoids rebuilding all native dependencies; it is local and ignored.

The existing frontend dependencies are referenced by a local `node_modules` junction. Source, generated Svelte files, Vite cache, native compilation output and app data are isolated. Do not run installation or update commands through this junction; install an independent dependency tree first if dependencies need changing.
