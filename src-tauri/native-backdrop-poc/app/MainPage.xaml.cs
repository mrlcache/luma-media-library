using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Composition.SystemBackdrops;

namespace HssBackdropPoc;

public sealed partial class MainPage : Page
{
    public MainPage()
    {
        InitializeComponent();
        Loaded += OnLoaded;
    }

    private async void OnLoaded(object sender, RoutedEventArgs e)
    {
        Loaded -= OnLoaded;
        var acrylicSupported = DesktopAcrylicController.IsSupported();
        NativeDiagnostic.Text =
            $"System Acrylic API supported: {acrylicSupported} · bounded region: " +
            $"{NativeBackdrop.ActualWidth:0} × {NativeBackdrop.ActualHeight:0} DIP. " +
            "API support does not prove visual rendering; compare against a controlled desktop background.";

        try
        {
            await HssWebContent.EnsureCoreWebView2Async();
            HssWebContent.CoreWebView2.NavigateToString(TransparentHssHtml);
        }
        catch (Exception error)
        {
            HssWebContent.Visibility = Visibility.Collapsed;
            System.Diagnostics.Debug.WriteLine($"WebView2 initialization failed: {error}");
        }
    }

    private const string TransparentHssHtml = """
        <!doctype html>
        <html lang="en">
        <head>
          <meta charset="utf-8">
          <meta name="viewport" content="width=device-width, initial-scale=1">
          <style>
            :root { color-scheme: dark; font-family: "Segoe UI Variable", "Segoe UI", sans-serif; }
            * { box-sizing: border-box; }
            html, body { width: 100%; height: 100%; margin: 0; background: transparent; color: #f2f4f7; }
            body { padding: 32px 48px; }
            h2 { margin: 0 0 22px; font-size: 22px; font-weight: 650; letter-spacing: -.035em; }
            .row { display: flex; gap: 20px; }
            .card { flex: 1; min-width: 0; }
            .poster { height: 150px; border-radius: 12px; background: linear-gradient(145deg,#273849,#131923 60%,#657887); }
            .card:nth-child(2) .poster { background: linear-gradient(145deg,#d5bd9e,#615143 55%,#23272d); }
            .card:nth-child(3) .poster { background: linear-gradient(145deg,#b3d3d9,#536b68 55%,#151a21); }
            .title { margin-top: 10px; font-size: 14px; font-weight: 600; }
            .meta { margin-top: 3px; color: #c0c7d0; font-size: 12px; }
            .note { margin-top: 22px; color: #d3d8df; font-size: 12px; }
          </style>
        </head>
        <body>
          <h2>Continue watching · WinUI 3 WebView2 diagnostic</h2>
          <div class="row">
            <article class="card"><div class="poster"></div><div class="title">The Last Signal</div><div class="meta">41m left</div></article>
            <article class="card"><div class="poster"></div><div class="title">North of Ordinary</div><div class="meta">S2 E3 · 28m left</div></article>
            <article class="card"><div class="poster"></div><div class="title">The Deep Hour</div><div class="meta">2024 · Thriller</div></article>
          </div>
          <p class="note">Diagnostic only: WinUI 3 XAML WebView2 does not support transparent backgrounds. An opaque result is expected and says nothing about the separate Win32 composition route.</p>
        </body>
        </html>
        """;
}
