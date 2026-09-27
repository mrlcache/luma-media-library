"""Render Luma's Fluxo mark with optical corrections for small app icons."""

from pathlib import Path
import tempfile

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[1]
INK = (234, 240, 242, 255)
BG = (17, 23, 25, 255)
SIZES = (16, 24, 32, 48, 64, 128, 256, 512)


def cubic(p0, p1, p2, p3, steps=80):
    points = []
    for i in range(1, steps + 1):
        t = i / steps
        u = 1 - t
        points.append((
            u**3 * p0[0] + 3 * u**2 * t * p1[0] + 3 * u * t**2 * p2[0] + t**3 * p3[0],
            u**3 * p0[1] + 3 * u**2 * t * p1[1] + 3 * u * t**2 * p2[1] + t**3 * p3[1],
        ))
    return points


def quadratic(p0, p1, p2, steps=60):
    return [
        ((1 - t)**2 * p0[0] + 2 * (1 - t) * t * p1[0] + t**2 * p2[0],
         (1 - t)**2 * p0[1] + 2 * (1 - t) * t * p1[1] + t**2 * p2[1])
        for t in (i / steps for i in range(1, steps + 1))
    ]


def render_icon(size):
    # Render each ICO/PNG size independently; a downscaled large icon loses the spark.
    sampling = 8 if size <= 64 else 4
    canvas = size * sampling
    image = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle((0, 0, canvas - 1, canvas - 1), radius=canvas * 9 / 32, fill=BG)
    if size <= 48:
        # Micro mark: retain the curved trail and spark without crossing tiny strokes.
        factor = canvas / 32
        origin = (0, 0)
        width = round((2.6 if size <= 16 else 2.2) * factor)
        runs = [[(5, 26)] + cubic((5, 26), (8.5, 20), (12.5, 16.5), (17, 14))]
        center = (22, 10.5)
        radius, core = 7, 2.6 if size <= 16 else 2
    else:
        # The full approved Fluxo symbol is retained at larger app-icon sizes.
        factor = canvas * 26 / (32 * 140)
        origin = ((canvas - 140 * factor) / 2 - 24 * factor,
                  (canvas - 111 * factor) / 2 - 23 * factor)
        width = round(6.5 * factor)
        runs = [
            [(128, 46)] + cubic((128, 46), (82, 20), (33, 20), (38, 42))
            + [(86, 128)] + quadratic((86, 128), (112, 108), (129, 79)),
            [(29, 121)] + cubic((29, 121), (57, 94), (94, 72), (126, 65)),
        ]
        center = (142, 64)
        radius, core = 19, 4.5

    def scaled(points):
        return [(round(origin[0] + x * factor), round(origin[1] + y * factor)) for x, y in points]

    for points in runs:
        line = scaled(points)
        draw.line(line, fill=INK, width=width, joint="curve")
        cap_radius = width / 2
        for x, y in (line[0], line[-1]):
            draw.ellipse((x - cap_radius, y - cap_radius, x + cap_radius, y + cap_radius), fill=INK)

    a, b = core * 0.7, core * 1.1
    star = [(0, -radius)]
    star += cubic((0, -radius), (a, -b), (b, -a), (radius, 0))
    star += cubic((radius, 0), (b, a), (a, b), (0, radius))
    star += cubic((0, radius), (-a, b), (-b, a), (-radius, 0))
    star += cubic((-radius, 0), (-b, -a), (-a, -b), (0, -radius))
    draw.polygon(scaled([(x + center[0], y + center[1]) for x, y in star]), fill=INK)
    return image.resize((size, size), Image.Resampling.LANCZOS)


def preview(before, icons):
    sheet = Image.new("RGB", (720, 300), BG[:3])
    draw = ImageDraw.Draw(sheet)
    draw.text((20, 15), "Favicon QA: actual size / pixel detail (3x)", fill=INK[:3])
    for column, size in enumerate((16, 24, 32)):
        x = 140 + column * 190
        draw.text((x, 40), f"{size}px", fill=INK[:3])
        for row, (name, icon) in enumerate((
            ("Before", before.resize((size, size), Image.Resampling.LANCZOS)),
            ("After", icons[size]),
        )):
            y = 75 + row * 110
            if column == 0:
                draw.text((20, y + 15), name, fill=INK[:3])
            sheet.paste(icon, (x, y + 25), icon)
            zoom = icon.resize((size * 3, size * 3), Image.Resampling.NEAREST)
            sheet.paste(zoom, (x + 50, y), zoom)
    path = Path(tempfile.gettempdir()) / "luma-favicon-qa.png"
    sheet.save(path)
    return path


def main():
    before = Image.open(ROOT / "src-tauri/icons/icon.png").convert("RGBA")
    icons = {size: render_icon(size) for size in SIZES}
    icons[512].save(ROOT / "src-tauri/icons/icon.png", optimize=True)
    for size in (16, 32):
        icons[size].save(ROOT / f"static/favicon-{size}.png", optimize=True)
    for target in (ROOT / "static/favicon.ico", ROOT / "src-tauri/icons/icon.ico"):
        icons[256].save(target, format="ICO", sizes=[(size, size) for size in SIZES if size <= 256],
                        append_images=[icons[size] for size in SIZES if size < 256])
    print(preview(before, icons))


if __name__ == "__main__":
    main()
