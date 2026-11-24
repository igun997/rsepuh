"""caption: the words of a sentence popping in one by one, the spoken one in the accent colour (spec: text, times, y, size)."""
from rs import *


def build(doc, sc, ctx):
    words = str(sc["text"]).split()
    size = ctx.px(sc.get("size", 104))
    max_w, gap = ctx.W * 0.86, size * 0.3
    times = sc.get("times")
    per = sc["dur"] / max(len(words), 1)
    widths = [measure(ctx.caps(sc, w), size, tracking=tracking(ctx)) for w in words]
    rows, row, rw = [], [], 0.0
    for i, w in enumerate(widths):
        if row and rw + gap + w > max_w:
            rows.append((row, rw))
            row, rw = [], 0.0
        row.append(i)
        rw += (gap if row[1:] else 0) + w
    rows.append((row, rw))
    line_h = size * 1.25
    y0 = ctx.H * sc.get("y", 0.66) - (len(rows) - 1) * line_h / 2
    for r, (idx, rw) in enumerate(rows):
        x = ctx.W / 2 - rw / 2
        for i in idx:
            ta = times[i] if times else i * per
            tb = (times[i + 1] if times and i + 1 < len(times) else sc["dur"]) if times else (i + 1) * per
            cx, cy = x + widths[i] / 2, y0 + r * line_h
            label = ctx.caps(sc, words[i])
            for hot in (False, True):
                layer = text(label, size=size, x=cx, y=cy, color=ctx.th("accent") if hot else ctx.th("fg"), preset="none", tracking=tracking(ctx), name=f"w{i}{'h' if hot else ''}")
                layer.scale = Anim(spring_scale(ta, 0.22, 72, 116 if hot else 110))
                layer.opacity = Anim([(ta, 0, "out"), (ta + 0.1, 100)])
                layer.start = ta
                layer.end = min(tb, sc["dur"]) if hot else sc["dur"]
                doc.add(layer)
            x += widths[i] + gap
