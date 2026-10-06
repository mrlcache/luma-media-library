# Local 1337x recovery

The configured 1337x indexer uses Prowlarr and its tagged FlareSolverr proxy. A stopped FlareSolverr causes connection-refused errors even while Prowlarr itself is running.

Before a live search, the connector checks the matching local proxy. If it is offline, it can start an existing Windows FlareSolverr executable, bound to loopback, and wait for readiness. Concurrent searches share the startup operation. Remote proxies are never started locally. Prowlarr must already be running and configured.

Development finds the executable next to the portable Prowlarr tools. An installed resolver can read its local executable path from `<appData>/flaresolverr-executable`; that machine-specific path is not shipped in the repository or APK.

Empty release lists are retried after 30 seconds rather than retained as successful results indefinitely. An empty cached list no longer masks a provider error. Previously saved nonempty lists and magnet links retain the existing offline fallback and ten-day sliding expiration.

Validation on 6 October 2026: live 1337x search returned 35 movie releases for The Batman, including QXR. The QXR opaque download resolved to a magnet without starting a download. Fixture tests cover cache recovery and preservation of usable offline results. Availability still depends on the upstream indexer.
