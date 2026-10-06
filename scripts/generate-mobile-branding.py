"""Use the reduced Luma mark with Android adaptive-icon safe padding."""
from pathlib import Path
from PIL import Image
import importlib.util
import sys

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('luma_icons', ROOT / 'scripts/generate-luma-icons.py')
icons = importlib.util.module_from_spec(spec)
spec.loader.exec_module(icons)
render_icon, BG = icons.render_icon, icons.BG
RES = ROOT / 'mobile/src-tauri/gen/android/app/src/main/res'
ADAPTIVE_SAFE_DP = 66
ADAPTIVE_MARK_DP = 56
FAVICON_MARK_FRACTION = 0.70
mark = render_icon(512, reduced=True, background=False)
mark = mark.crop(mark.getbbox())
for density, scale in [('mdpi', 1), ('hdpi', 1.5), ('xhdpi', 2), ('xxhdpi', 3), ('xxxhdpi', 4)]:
    # Keep 5 dp of breathing room inside Android's centered 66 dp safe zone.
    edge = round(108 * scale)
    glyph = mark.copy()
    glyph.thumbnail((round(ADAPTIVE_MARK_DP * scale), round(ADAPTIVE_MARK_DP * scale)), Image.Resampling.LANCZOS)
    foreground = Image.new('RGBA', (edge, edge), (0, 0, 0, 0))
    foreground.alpha_composite(glyph, ((edge-glyph.width)//2, (edge-glyph.height)//2))
    directory = RES / f'mipmap-{density}'
    foreground.save(directory / 'ic_launcher_foreground.png', optimize=True)
    legacy = Image.new('RGBA', (edge, edge), BG)
    legacy.alpha_composite(foreground)
    legacy = legacy.resize((round(48*scale), round(48*scale)), Image.Resampling.LANCZOS)
    for name in ['ic_launcher.png', 'ic_launcher_round.png']:
        legacy.save(directory / name, optimize=True)
    bbox = foreground.getbbox()
    assert bbox and bbox[2]-bbox[0] <= round(ADAPTIVE_SAFE_DP*scale) and bbox[3]-bbox[1] <= round(ADAPTIVE_SAFE_DP*scale)
print('Android reduced mark generated inside the adaptive safe area.')

# Browser fallbacks match the same reduced SVG, with room around the mark.
for edge in [16, 32]:
    foreground = render_icon(512, reduced=True, background=False)
    foreground = foreground.crop(foreground.getbbox())
    foreground.thumbnail((round(edge*FAVICON_MARK_FRACTION), round(edge*FAVICON_MARK_FRACTION)), Image.Resampling.LANCZOS)
    # Use only the background from the renderer, then place the padded glyph.
    from PIL import ImageDraw
    icon = Image.new('RGBA', (edge, edge), (0, 0, 0, 0))
    ImageDraw.Draw(icon).rounded_rectangle((0, 0, edge-1, edge-1), radius=edge*9/32, fill=BG)
    icon.alpha_composite(foreground, ((edge-foreground.width)//2, (edge-foreground.height)//2))
    icon.save(ROOT / f'static/favicon-{edge}.png', optimize=True)
