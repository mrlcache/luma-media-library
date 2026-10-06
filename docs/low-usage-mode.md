# Low Usage Mode

Enable **Settings → Background server → Low Usage Mode** on Windows. Closing the
main window then destroys the desktop webview instead of quitting Luma. The tray
menu offers **Open Luma** and **Quit Luma**. Opening the usual shortcut also restores
the running instance rather than launching a second server.

The mobile bridge, media server, active transcodes, torrent engine and library
workers remain available. The desktop native player stops after saving its
position, and the desktop artwork cache is released and disabled until the window
is reopened. This saves interface memory; active transcoding still needs CPU and
memory, and the PC must remain awake.

The setting persists in the application profile. If the tray cannot be created,
background mode is not enabled. An explicit quit still exits the application.
For background startup, launch Luma with `--background` after enabling the mode.
`--quit` tells an already running instance to exit.

Desktop seeking now previews the target immediately, coalesces rapid commands,
commits slider seeks on release, and rejects status responses from before the
latest seek. Playback progress is saved only after the seek position is confirmed.

## Validation

- Frontend type checking passed with no errors.
- Seek regression tests cover stale polls, successive seeks, backward seeks and
  switching titles.
- Windows background startup was tested against the real application profile:
  no desktop WebView child remained, the bridge health and authenticated library
  requests returned HTTP 200, a repeated launch reused the process, and explicit
  quit stopped it. The previous mode preference was restored after the test.
- Active streaming and downloads retain their existing backend services; live
  playback on a physical phone was not part of this background smoke test.
