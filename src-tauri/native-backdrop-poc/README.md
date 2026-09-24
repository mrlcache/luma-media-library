# Native backdrop composition POC

These isolated samples test two distinct composition routes. They are not
part of the product and must not be used to migrate the production Tauri shell
without visual and interaction evidence.

## Current production candidate (static source audit)

The current app obtains the HWND from its existing Tauri `WebviewWindow` and
sets `DWMWA_SYSTEMBACKDROP_TYPE` on that same HWND via the main UI thread. This
path creates no helper/overlay window. Desktop CSS leaves the shell transparent,
keeps the sidebar's base gradient opaque, and applies a low-opacity tint only to
HSS. These facts verify the intended composition path in source, not that
WebView2 actually reveals Acrylic at runtime.

The installed Tao 0.35.3 source shows that Tauri's `transparent: true` window
uses `DwmEnableBlurBehindWindow` with an empty region unless
`no_redirection_bitmap` is enabled. Microsoft says this legacy call no longer
produces blur beginning with Windows 8. This identifies the alpha/transparency
path but does not prove that it conflicts with the modern DWM system backdrop.
The WRY baseline now has an isolated no-redirection mode to test that difference.

`app/` is a WinUI 3 / Windows App SDK 2.5.1 test: opaque XAML sidebar and hero,
with one bounded `SystemBackdropElement` spanning the HSS test region. The
region's left half is XAML-only; the user visually confirmed that native
Desktop Acrylic works in this bounded gray HSS region. The right overlays
transparent WebView2, testing whether its alpha reveals the same material
without introducing a seam between adjacent Acrylic elements.
It now also refuses to create a window unless its process receives the explicit
`--visual-test` argument. The user approved a visual run on 2026-09-23. The
current single-element layout was built with .NET SDK 10.0.401 using one
MSBuild worker and launched through the Windows App Development CLI. Microsoft
documents the region-bounded `SystemBackdropElement`, but separately warns that WinUI 3 does not support
transparent WebView2 backgrounds. A successful build is not evidence of the
visual effect; the user confirmation applies only to the native XAML half. The
limitation is specific to the WinUI 3 XAML control: WebView2's Win32 controller supports a
fully transparent background on supported Windows versions. Thus the XAML-only
half is the supported bounded-material test, while its WebView2 half is only a
diagnostic of the documented unsupported combination; it must not be used to
rule out the separate Win32/DirectComposition route. The user noted that the
gradients around the gray test region may not be visible; this is distinct from
the native material result and remains to be checked in the actual Svelte HSS.

A read-only host check on 2026-09-23 found Windows 11 build 26200, system
transparency enabled (`EnableTransparency=1`), Energy Saver off while on
battery (44%), High Contrast off, Balanced power plan, and Intel Iris Xe
graphics (driver 30.0.101.2079; 8 GiB RAM). These checks make the common
Windows policy fallbacks less likely. The native XAML region is now visually
confirmed, but the production Tauri/DWM route remains a separate test. Keep the
existing app/server session untouched; benchmark GPU/RAM and validate pointer,
keyboard/focus, resize, DPI and fallback before considering production. WRY's
DirectComposition-hosting PR remains open and unreviewed, so it is a research
lead, not a dependency to ship.

`wry/` is a small current-WRY + DWM baseline test, not a DirectComposition
hosting test. It has three manually selectable modes: the current transparent
top-level HWND plus transparent WebView2 (including Tao's legacy transparency
setup), opaque top-level HWND plus transparent WebView2, and a transparent
top-level HWND with Tao's `WS_EX_NOREDIRECTIONBITMAP` path enabled (which skips
its legacy `DwmEnableBlurBehindWindow` call). The third mode is an isolated A/B
diagnostic, not an assumed fix. The HSS tint uses roughly the same 30–38%
opacity as the current desktop UI so it does not hide most of the native
material. The transparent mode matches the app's transparency flags, but keeps
the native title bar as a test control; the product's frameless resize behavior
remains a separate check. The title bar exposes standard move, resize, minimize
and maximize/restore behavior; in-page click, keyboard-focus and resize
indicators help verify WebView input. It refuses to create any window unless passed
`--visual-test` and exactly one of `--window-transparent`,
`--window-transparent-no-redirection`, or `--window-opaque`. This comparison can
reveal whether top-level host transparency or Tao's legacy setup changes the
DWM backdrop behavior; it does not presume a Tao/DWM conflict.
This direct Tao/WRY test isolates window creation but does not include
`tauri-runtime-wry`'s extra softbuffer transparent-clear surface, so even a
positive result must be reproduced in Tauri before changing production.
Its Rust dependency check was stopped by Windows virtual-memory exhaustion
while compiling the generated `windows` crate; it has not been built or run.
The updated Rust source passes `rustfmt --check`; that is a lightweight syntax/
formatting check, not a build or validation of the WRY dependency path.

A separate route merits an isolated POC if the simple DWM HWND material is
visually insufficient: host Svelte through Win32 WebView2's
`CoreWebView2CompositionController`, attach its WebView visual to a composition
tree, and place a clipped host-backdrop/effect visual under it for HSS. Microsoft
documents transparent WebView2 backgrounds and composition visual attachment;
Windows documents `DWMWA_USE_HOSTBACKDROPBRUSH` and
`Compositor.CreateHostBackdropBrush` for host transparency effects in non-UWP
Win32 applications. These APIs have not been combined or tested here. The
approach would replace WRY's current controller integration and require native
input/focus/accessibility/DPI/window-lifecycle handling; it is not a production
change or a proven low-memory route.

The composition prototype should specifically use `CreateHostBackdropBrush`
for pixels behind the top-level HWND; `CreateBackdropBrush` samples the app's
own composition scene instead. Enable host backdrop sampling with
`DWMWA_USE_HOSTBACKDROPBRUSH`, feed it through a bounded Gaussian-blur/tint
effect visual sized to the HSS, then attach the transparent WebView2 composition
visual above it. This gives the blur a tunable radius and a native visual bound,
unlike `DWMWA_SYSTEMBACKDROP_TYPE`, which selects a fixed system material for the
whole window. Microsoft's APIs document these pieces individually, not this
combined WebView2/Tauri route, so it remains an isolated visual prototype until
it passes rendering, interaction, resize/DPI, and low-memory checks.

Do not base that POC on the WinUI 3 XAML WebView2 control: transparent
backgrounds are unsupported there. There is a second, separate interop risk:
`CoreWebView2CompositionController.RootVisualTarget` documents
`IDCompositionVisual` / `Windows.UI.Composition.ContainerVisual`, while WinUI 3
uses `Microsoft.UI.Composition.Visual`. A Microsoft WebView2 feedback report
describes this type mismatch and does not establish a supported bridge. Treat
that as an unresolved compatibility warning, not proof that bridging is
impossible. The official pure Win32 `WebView2SampleWinComp` is concrete
precedent for the compatible `Windows.UI.Composition` route: it creates one
composition WebView, queries the ordinary controller from that same COM object,
sets `RootVisualTarget`, updates controller bounds on resize, and forwards
mouse messages. This demonstrates WebView visual hosting in a Win32 app, not
our bounded HSS blur: the sample has no host-backdrop effect. If the DWM
baseline needs replacement, start from that sample (or another documented
compatible host), not WinUI 3 XAML WebView2, and keep it isolated. Do not create
a hidden duplicate WebView; that would increase memory and would not test the
production path.

There may be a smaller path for this Tauri project than replacing its host
with standalone C++: WRY PR [#1762](https://github.com/tauri-apps/wry/pull/1762)
adds opt-in DirectComposition hosting and a per-HWND registration API for
embedders such as `tauri-runtime-wry`. It targets the existing HWND, avoids the
child WebView HWND, reuses WRY's normal initialization path, and contains
mouse/pointer, focus, cursor, bounds and move handling. However, it remains an
open, unreviewed PR. Its Windows test and Clippy CI jobs succeeded, but its
composition-specific visual runtime was not tested by CI; its manual UAT is
from a production fork, not this application. This project locks WRY 0.55.1, which
predates the unmerged route. Tauri config-created windows are built before the
app `setup` callback, but WRY registration must happen before WebView creation.
Tauri 2.11 exposes `Window::add_child` only behind its `unstable` feature, so
an app-side experiment would need a manually created bare window plus
pre-registration before attaching the one WebView, or a Tauri runtime hook.
The registration map is thread-local inside WRY, so the app's registration
function must come from the same globally patched WRY package instance used by
`tauri-runtime-wry`, and run on the UI thread; a separate WRY dependency from a
different source/version would not share the pending target.
This is promising but not a drop-in toggle; verify the Tauri lifecycle and
upstream status before implementing.

The current WinUI revision has been built and launched with explicit user
approval; keep that test window and the existing dev server untouched. The WRY
baseline remains unbuilt because its Cargo check exhausted virtual memory while
compiling generated Windows bindings. Do not retry that build while memory is
pressured. The WinUI screenshot is inconclusive: the native half appears dark
and uniform, and the WebView2 half is an explicitly unsupported diagnostic.
Do not claim Acrylic works until a controlled visual comparison confirms it.
Future review should check Acrylic visibility, opaque sidebar/hero, HSS tint,
text/input interaction, window behavior and the fallback. Windows may replace
background Acrylic with a solid fallback while inactive; verify restoration on
reactivation rather than treating that expected behavior as composition failure.
If WebView2 occludes Acrylic or causes glitches, record that as a failed
interop route rather than shipping a workaround based on private APIs.

Relevant platform constraints:

- `DWMWA_SYSTEMBACKDROP_TYPE` / Desktop Acrylic targets Windows 11 build 22621+
  and applies behind the entire HWND, so opaque pixels must mask non-HSS areas.
  The attribute selects a system material kind; it has no blur-radius control.
  HSS turns CSS `backdrop-filter` off in desktop mode, so increasing a CSS blur
  cannot strengthen the actual desktop Acrylic. An adjustable radius needs a
  custom Composition effect and must be benchmarked for GPU/memory cost. The
  current desktop HSS tint was reduced from 0.66–0.72 to 0.30–0.38 opacity,
  with a subtler gloss, to reveal more of the native material; this increases
  its visibility, not the DWM blur radius. That visual change still needs a
  runtime check.
- WinUI `SystemBackdropElement` scopes a system backdrop to its XAML bounds, but
  transparency between that XAML and WinUI 3 WebView2 is explicitly unsupported
  in Microsoft's WebView2 documentation. Separately, the WebView2 Win32
  controller supports a fully transparent default background (alpha 0); don't
  generalize the WinUI limitation to the Win32/DirectComposition host.
- Microsoft's Win32 `WebView2SampleWinComp` source demonstrates the lower-level
  alternative: one CompositionController, ordinary controller queried from the
  same COM object, `Windows.UI.Composition` visual tree, resize bounds updates,
  and forwarded mouse input. It does not demonstrate a host-backdrop blur. The
  WinUI3 `Microsoft.UI.Composition.Visual` interop concern is specific to that
  composition type and does not block using the documented Win32 sample route.
- Microsoft's Acrylic guidance recommends Desktop Acrylic for transient
  surfaces rather than large persistent backgrounds. The requested HSS use is
  intentionally larger, so test visible quality and composition cost rather
  than assume the stock material is ideal. Acrylic is GPU-intensive and Windows
  can replace it with a solid color when transparency effects are disabled,
  Battery Saver is active, or hardware is considered low-end; the UI must remain
  legible in that fallback.
- Microsoft references: [System backdrops and bounded `SystemBackdropElement`](https://learn.microsoft.com/en-us/windows/apps/develop/ui/system-backdrops), [WinUI 3 WebView2 transparency limitation](https://learn.microsoft.com/en-us/microsoft-edge/webview2/platforms/winui3-windows-app-sdk), [Win32 WebView2 transparent background](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controller2), [WebView2 composition rendering](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/overview-features-apis#rendering-webview2-using-composition), [`DWMWA_USE_HOSTBACKDROPBRUSH`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute), and [Acrylic guidance](https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic).

## App-owned HSS direction (2026-09-23)

The requirement is an effect owned by this application. Do not change the Windows
wallpaper, system colors, transparency settings, or power settings. A host
backdrop brush only samples the scene behind its visual; when the desktop is
actually behind the app, the wallpaper may be visible through the HSS, but the
app must never set or replace it. Remove the current
`DWMWA_SYSTEMBACKDROP_TYPE` whole-window route from any eventual production
implementation: it cannot independently bound the material to the HSS.

The current Tauri config creates its main WebView before the Rust `setup`
callback, so the composition target cannot be registered there in time. Tauri's
unstable `Window::builder` + `Window::add_child` path can instead create a bare
native window first, set up its composition tree, register the target, and then
create the existing app WebView. This keeps the Svelte UI and its single native
window, but changes its construction from `WebviewWindow` to a bare `Window`
plus one child `Webview`; window APIs and the window-state plugin must be
checked before adopting it.

Source inspection confirms the window-state plugin listens to generic `Window`
move/resize/close events and stores state by window label, so the bare-window
route is not immediately blocked by that plugin. The product's HSS command
currently accepts `WebviewWindow`; it would need to accept the underlying
`Window` instead. The manually created child Webview must retain the `main`
label and app URL so the existing frontend IPC and window APIs keep resolving.

The required WRY composition-hosting implementation is available as open PR
[tauri-apps/wry #1762](https://github.com/tauri-apps/wry/pull/1762), commit
`753e11129c1781498e6a7f01a14628db5a3550a1`. It adds an opt-in
`ICoreWebView2CompositionController`, keeps normal WRY initialization shared,
and forwards mouse, focus, cursor, resize, and pointer events. It is not merged
upstream, so use it only in an isolated POC pinned to that exact commit; do not
silently ship a floating fork dependency.

The POC should create one `Windows.UI.Composition` tree for the existing HWND:

1. Keep the sidebar and hero opaque in the WebView.
2. Create one host-backdrop/effect visual clipped to the HSS bounds and place it
   below a transparent composition-hosted WebView visual.
3. Do not set `DWMWA_SYSTEMBACKDROP_TYPE` and do not create a second top-level
   window.
4. Only after an explicitly approved visual run, test HSS-only bounds, resize,
   DPI, input/focus, inactive-window fallback, and GPU/RAM cost before deciding
   whether to port this construction into the product.

This route is technically actionable, not yet implemented or visually proven.
The machine currently reports about 1.5 GiB free physical memory, and an earlier
Rust Windows-bindings check exhausted virtual memory. Do not start Cargo builds
while memory is this constrained; continue source-level POC work and build only
when there is enough headroom. Keep the running app and dev-server session
untouched.
