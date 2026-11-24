"""Extract vector outlines of a font into a small JSON file the Lottie kit turns into shapes.

    python3 extract_glyphs.py Inter-Bold.ttf ../fonts/inter-bold.json

Text in Lottie is most reliable as shapes: no font has to exist on the player (Skottie) or in the
renderer (EffectCraft), and every letter can be animated on its own. Outlines are scaled to a
1000 unit em with y pointing down (Lottie's convention), as cubic Beziers. Needs fontTools.
"""
import json
import sys

from fontTools.pens.basePen import decomposeQuadraticSegment
from fontTools.pens.recordingPen import DecomposingRecordingPen
from fontTools.ttLib import TTFont

CHARS = [chr(c) for c in range(32, 127)] + list("…–—‘’“”•·→←↑↓✓×")


def contours(rec, scale):
    """RecordingPen ops -> list of contours; each is a list of cubic segments (p0, c1, c2, p1)."""
    out, cur, start, pos = [], None, None, None
    f = lambda p: (p[0] * scale, -p[1] * scale)
    for op, pts in rec.value:
        if op == "moveTo":
            cur, start = [], f(pts[0])
            pos = start
        elif op == "lineTo":
            p = f(pts[0])
            cur.append((pos, pos, p, p))
            pos = p
        elif op == "qCurveTo":
            pts = [f(p) for p in pts]
            for q, end in decomposeQuadraticSegment(pts):
                c1 = (pos[0] + 2 / 3 * (q[0] - pos[0]), pos[1] + 2 / 3 * (q[1] - pos[1]))
                c2 = (end[0] + 2 / 3 * (q[0] - end[0]), end[1] + 2 / 3 * (q[1] - end[1]))
                cur.append((pos, c1, c2, end))
                pos = end
        elif op == "curveTo":
            c1, c2, end = (f(p) for p in pts)
            cur.append((pos, c1, c2, end))
            pos = end
        elif op in ("closePath", "endPath"):
            if cur is not None and pos != start:
                cur.append((pos, pos, start, start))
            if cur:
                out.append(cur)
            cur = None
    return out


def to_lottie(contour):
    v, i, o = [], [], []
    n = len(contour)
    for k, (p0, c1, c2, p1) in enumerate(contour):
        prev = contour[k - 1]
        v.append([round(p0[0], 2), round(p0[1], 2)])
        # in-tangent: from previous segment's second control to this vertex; out-tangent: first control
        i.append([round(prev[2][0] - p0[0], 2), round(prev[2][1] - p0[1], 2)])
        o.append([round(c1[0] - p0[0], 2), round(c1[1] - p0[1], 2)])
    return {"v": v, "i": i, "o": o, "c": True}


def main(src, dst):
    font = TTFont(src)
    gs, cmap, upm = font.getGlyphSet(), font.getBestCmap(), font["head"].unitsPerEm
    scale = 1000.0 / upm
    hhea, os2 = font["hhea"], font["OS/2"]
    data = {"upm": 1000, "ascender": round(hhea.ascent * scale, 1), "descender": round(hhea.descent * scale, 1),
            "capHeight": round(getattr(os2, "sCapHeight", 700) * scale, 1), "xHeight": round(getattr(os2, "sxHeight", 520) * scale, 1),
            "glyphs": {}}
    for ch in CHARS:
        name = cmap.get(ord(ch))
        if name is None:
            continue
        g = gs[name]
        pen = DecomposingRecordingPen(gs)
        g.draw(pen)
        data["glyphs"][ch] = {"adv": round(g.width * scale, 1), "paths": [to_lottie(c) for c in contours(pen, scale)]}
    with open(dst, "w") as fh:
        json.dump(data, fh, separators=(",", ":"))
    print(f"{len(data['glyphs'])} glyphs, {sum(len(g['paths']) for g in data['glyphs'].values())} contours -> {dst}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
