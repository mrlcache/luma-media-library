use std::env;
#[cfg(windows)]
use tao::platform::windows::WindowBuilderExtWindows;
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

#[cfg(windows)]
fn set_desktop_acrylic(hwnd: isize) -> Result<(), String> {
    use std::ffi::c_void;
    use std::mem::size_of_val;
    use windows_sys::Win32::{
        Foundation::HWND,
        Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE},
    };

    const DWMSBT_TRANSIENTWINDOW: u32 = 3;
    let hwnd = hwnd as HWND;
    let backdrop_type = DWMSBT_TRANSIENTWINDOW;
    let result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE as u32,
            &backdrop_type as *const _ as *const c_void,
            size_of_val(&backdrop_type) as u32,
        )
    };

    if result < 0 {
        return Err(format!(
            "DwmSetWindowAttribute failed: HRESULT 0x{:08X}",
            result as u32
        ));
    }
    Ok(())
}

fn main() -> wry::Result<()> {
    let args: Vec<String> = env::args().collect();
    let wants_visual_test = args.iter().any(|arg| arg == "--visual-test");
    let transparent_window = args.iter().any(|arg| arg == "--window-transparent");
    let no_redirection_window = args
        .iter()
        .any(|arg| arg == "--window-transparent-no-redirection");
    let opaque_window = args.iter().any(|arg| arg == "--window-opaque");
    let selected_modes = [transparent_window, no_redirection_window, opaque_window]
        .into_iter()
        .filter(|selected| *selected)
        .count();

    // Safe by default: a build or accidental run must never open a window.
    if !wants_visual_test || selected_modes != 1 {
        println!("No window opened. When the screen is free, pass --visual-test and exactly one of --window-transparent, --window-transparent-no-redirection, or --window-opaque.");
        return Ok(());
    }

    let transparent = !opaque_window;
    let title = if no_redirection_window {
        "HSS Acrylic composition test · no redirection bitmap"
    } else if opaque_window {
        "HSS Acrylic composition test · opaque baseline"
    } else {
        "HSS Acrylic composition test · legacy transparent baseline"
    };

    let event_loop = EventLoop::new();
    let window_builder = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(tao::dpi::LogicalSize::new(1280.0, 800.0))
        // Keep native chrome so the visual check can exercise OS move, resize,
        // minimize, and maximize behavior without custom overlay controls.
        .with_decorations(true)
        .with_transparent(transparent);

    // This skips Tao's legacy DwmEnableBlurBehindWindow transparency path.
    // It is an isolated A/B mode, not an assumed fix.
    #[cfg(windows)]
    let window_builder = window_builder.with_no_redirection_bitmap(no_redirection_window);

    let window = window_builder
        .build(&event_loop)
        .expect("could not create the isolated test window");

    #[cfg(windows)]
    if let Err(error) = set_desktop_acrylic(tao::platform::windows::WindowExtWindows::hwnd(&window))
    {
        eprintln!("Native Acrylic setup failed: {error}");
    }

    let webview = WebViewBuilder::new()
        .with_transparent(true)
        .with_html(TEST_HTML)
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {
                let _ = &webview;
            }
        }
    });
}

const TEST_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>
    * { box-sizing: border-box; }
    html, body { width: 100%; height: 100%; margin: 0; background: transparent; color: #f1f3f5; font-family: "Segoe UI Variable", "Segoe UI", sans-serif; }
    .shell { height: 100vh; display: grid; grid-template-columns: 224px minmax(0, 1fr); grid-template-rows: 36px 286px minmax(0, 1fr); }
    .sidebar { grid-column: 1; grid-row: 1 / 4; padding: 32px 22px; background: #171a1f; border-right: 1px solid #ffffff1c; }
    .brand { margin-bottom: 34px; font-size: 19px; font-weight: 650; }
    .nav { display: grid; gap: 18px; color: #cbd1d8; font-size: 14px; }
    .nav span:last-child { position: absolute; bottom: 28px; }
    .chrome { grid-column: 2; grid-row: 1; padding: 9px 24px; background: #0a0c10; color: #9da5af; font-size: 12px; }
    .hero { grid-column: 2; grid-row: 2; display: flex; padding: 35px 48px; flex-direction: column; justify-content: center; background: linear-gradient(90deg, #090b0f 0%, #10141a 50%, #080a0d 100%); }
    h1 { margin: 0 0 12px; font-size: 40px; letter-spacing: -.045em; }
    .meta, .copy { color: #bac2ca; font-size: 13px; }
    .copy { max-width: 520px; margin: 13px 0 18px; line-height: 1.55; }
    .actions { display: flex; gap: 10px; align-items: center; }
    .actions button, .hss button { padding: 10px 16px; border: 1px solid #ffffff22; border-radius: 9px; background: #e5e9ec; color: #11151a; font-weight: 650; cursor: pointer; }
    .actions input { min-width: 180px; padding: 10px 12px; border: 1px solid #ffffff30; border-radius: 8px; background: #10151bcc; color: #f1f3f5; }
    .hss { grid-column: 2; grid-row: 3; padding: 32px 48px 40px; border-top: 1px solid #ffffff20; background: linear-gradient(145deg, #20262d61, #11161c4d); }
    .hss-heading { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
    h2 { margin: 0 0 22px; font-size: 21px; font-weight: 650; letter-spacing: -.035em; }
    .row { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 18px; }
    .card { min-width: 0; }
    .poster { height: 150px; border-radius: 11px; background: linear-gradient(135deg, #26384a, #10151d 62%, #778b9a); }
    .card:nth-child(2) .poster { background: linear-gradient(135deg, #d6bda2, #625448 55%, #20252a); }
    .card:nth-child(3) .poster { background: linear-gradient(135deg, #b9d4d4, #536b68 58%, #14191e); }
    .title { margin-top: 9px; font-size: 14px; font-weight: 600; }
    .sub { margin-top: 3px; color: #bbc2c9; font-size: 12px; }
    .status { position: absolute; right: 22px; bottom: 18px; color: #c8d4db; font-size: 11px; }
  </style>
</head>
<body>
  <main class="shell">
    <aside class="sidebar">
      <div class="brand">◈ FRAME</div>
      <nav class="nav"><span>⌂　Home</span><span>▣　Library</span><span>▤　Movies</span><span>▣　Series</span><span>⚙　Settings</span></nav>
    </aside>
    <div class="chrome">Home <span style="float:right">⌕　Search　　 ◉　 M</span></div>
    <section class="hero">
      <h1>THE LAST SIGNAL</h1>
      <div class="meta">98% match　·　2024　·　1h 52m　·　Sci-fi drama</div>
      <p class="copy">Opaque hero surface. The desktop backdrop should remain masked here and in the sidebar.</p>
      <div class="actions">
        <button id="play-test" type="button">▶　Play</button>
        <button id="details-test" type="button">ⓘ　Details</button>
        <input id="keyboard-test" type="text" aria-label="Keyboard input test" placeholder="Type to test keyboard focus">
      </div>
    </section>
    <section class="hss">
      <div class="hss-heading"><h2>Continue watching</h2><button id="pointer-test" type="button">Test click</button></div>
      <div class="row">
        <article class="card"><div class="poster"></div><div class="title">The Last Signal</div><div class="sub">41m left</div></article>
        <article class="card"><div class="poster"></div><div class="title">North of Ordinary</div><div class="sub">S2 E3 · 28m left</div></article>
        <article class="card"><div class="poster"></div><div class="title">The Deep Hour</div><div class="sub">2024 · Thriller</div></article>
      </div>
    </section>
    <div class="status" id="status" aria-live="polite">Transparent WebView2 + DWM Acrylic · HSS surface</div>
  </main>
  <script>
    const status = document.getElementById('status');
    const report = message => { status.textContent = message; };
    document.getElementById('play-test').addEventListener('click', () => report('Click OK · Play'));
    document.getElementById('details-test').addEventListener('click', () => report('Click OK · Details'));
    document.getElementById('pointer-test').addEventListener('click', () => report('Click OK · HSS'));
    document.getElementById('keyboard-test').addEventListener('input', () => report('Keyboard input OK'));
    window.addEventListener('focus', () => report('Window focus OK'));
    window.addEventListener('blur', () => report('Window focus lost'));
    window.addEventListener('resize', () => report(`Resize OK · ${innerWidth} × ${innerHeight}`));
  </script>
</body>
</html>"#;
