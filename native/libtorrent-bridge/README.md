# Native libtorrent bridge

This is the direct libtorrent-rasterbar backend for the Torrents page. It does
not use qBittorrent. The C ABI in `include/media_torrent_bridge.h` is kept
small so the Tauri/Rust process can load the engine without exposing C++ types.

Current implementation: magnet and `.torrent` imports, transfer snapshots,
pause/resume, queue movement, session speed limits, removal that preserves
downloaded files, and per-torrent fast-resume data. No torrent is started by
the smoke test.

Build on Windows with Visual Studio C++ tools, CMake and vcpkg:

```powershell
cmake -S . -B build -DCMAKE_TOOLCHAIN_FILE=<vcpkg-root>/scripts/buildsystems/vcpkg.cmake -DVCPKG_TARGET_TRIPLET=x64-windows-static-md
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure
```

The Tauri app loads the bridge on first use, saves resume data every minute and
on shutdown, and bundles the DLL under `resources/torrent-engine`. The Torrents
page reads live snapshots and sends queue, pause, speed limit, magnet, and
`.torrent` file actions directly to libtorrent. The smoke test starts an empty
session only; it does not add a torrent or contact trackers.

The bridge must never delete media files on removal. A separate explicit
delete-data operation, if ever added, requires its own confirmation flow.
