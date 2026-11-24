"""diagram: React (an atom) above, the outside world (a globe) below, and a two-way flow between them.

  { "type": "custom", "file": "lottie/scenes/diagram.py", "dur": 12,
    "top":    { "title": "React", "items": ["state", "props", "JSX"] },
    "bottom": { "title": "Outside world", "items": ["page title", "network", "timers"] },
    "bridge": "useEffect", "topBeat": 1, "bottomBeat": 5, "arrowBeat": 9 }

Everything draws itself on (Lottie trim paths), then keeps moving: the atom spins, the globe sways,
dots travel along the arrows and the bridge pulses.
"""
import math
from rs import *


def tag_row(ctx, items, cy, at0, dur):
    size = ctx.px(44)
    widths = [measure(i, size) + ctx.px(76) for i in items]
    rows, row, rw, gap, max_w = [], [], 0.0, ctx.px(20), ctx.W * 0.88
    for k, w in enumerate(widths):
        if row and rw + gap + w > max_w:
            rows.append((row, rw))
            row, rw = [], 0.0
        row.append(k)
        rw += (gap if row[1:] else 0) + w
    rows.append((row, rw))
    rh, rg = ctx.px(86), ctx.px(16)
    y0 = cy - (len(rows) - 1) * (rh + rg) / 2
    n = 0
    for r, (idx, w) in enumerate(rows):
        x = ctx.W / 2 - w / 2
        for k in idx:
            cx, cyy = x + widths[k] / 2, y0 + r * (rh + rg)
            tl = text(items[k], size=size, x=0, y=0, color=ctx.th("fg"), preset="none")
            t = at0 + n * 0.12
            pos = Anim([(t, (cx, cyy + ctx.px(26)), "out"), (t + 0.5, (cx, cyy), "out")] + idle_keys((cx, cyy), t + 0.5, dur, 0, ctx.px(4), 3.2 + n * 0.3, n)[1:])
            doc_layer = Layer(tl.shapes + [group([rect((widths[k], rh), (0, 0), rh / 2), fill(ctx.th("panel"))], name="tag")], name="tag", pos=pos,
                              scale=Anim(spring_scale(t, 0.5, 60, 108)), opacity=Anim([(t, 0, "out"), (t + 0.2, 100)]), start=t)
            yield doc_layer
            x += widths[k] + gap
            n += 1


def heading(ctx, title, y, color, at):
    size = fit_size(ctx, [title], ctx.px(92), ctx.W * 0.8, cap=ctx.px(96))
    yield rise_layer(text(title, size=size, x=ctx.W / 2, y=y, color=color, preset="none", name="heading"), at, 0.6, ctx.px(50))
    yield underline(ctx, ctx.W / 2, y + size * 0.36 + ctx.px(24), min(measure(title, size) * 0.45, ctx.W * 0.3), color, at + 0.3, 0.5)


def atom(ctx, x, y, color, at, dur):
    rx, ry = ctx.px(128), ctx.px(48)
    for k in range(3):
        base = k * 60
        draw = Anim([(at + k * 0.14, 0, "out"), (at + k * 0.14 + 0.9, 100)])
        spin = Anim([(0, base, "linear"), (dur, base + 24 * dur)])
        yield Layer([group([ellipse((rx * 2, ry * 2)), trim(0, draw), stroke(color, ctx.px(8))], rot=spin, name="orbit")], name=f"orbit-{k}", pos=(x, y), start=at)
    yield Layer([group([ellipse(ctx.px(44)), fill(color)])], name="nucleus", pos=(x, y), scale=Anim(spring_scale(at + 0.2, 0.5, 0, 120)), start=at + 0.2)


def globe(ctx, x, y, color, at, dur):
    r = ctx.px(112)
    specs = [(r * 2, r * 2, None), (ctx.px(96), r * 2, 0.0), (r * 2, ctx.px(76), None), (ctx.px(164), r * 2, math.pi / 2)]
    for k, (w, h, phase) in enumerate(specs):
        draw = Anim([(at + k * 0.14, 0, "out"), (at + k * 0.14 + 0.9, 100)])
        if phase is not None:  # a meridian that narrows and widens: the globe turning
            keys = [(t / 2.0, (100 * (0.25 + 0.75 * abs(math.cos(math.pi * t / 6.0 + phase))), 100), "inout") for t in range(0, int(dur * 2) + 2)]
            sc = Anim(keys)
        else:
            sc = (100, 100)
        yield Layer([group([ellipse((w, h)), trim(0, draw), stroke(color, ctx.px(8))], scale=sc, name="line")], name=f"globe-{k}", pos=(x, y), start=at)


def arrow(ctx, x, ya, yb, color, at, dur):
    tip = 1 if yb > ya else -1
    line = polyline_layer([(x, ya), (x, yb)], color, ctx.px(9), Anim([(at, 0, "out"), (at + 0.5, 100)]), at)
    head = polyline_layer([(x - ctx.px(30), yb - tip * ctx.px(36)), (x, yb), (x + ctx.px(30), yb - tip * ctx.px(36))], color, ctx.px(9), Anim([(at + 0.35, 0, "out"), (at + 0.6, 100)]), at + 0.35)
    yield line
    yield head
    # a dot that keeps travelling along the arrow
    cyc, t0 = 1.5, at + 0.9
    pk, ok, t = [], [], t0
    while t < dur:
        pk += [(t, (x, ya), "inout"), (t + cyc, (x, yb), "inout")]
        ok += [(t, 0, "inout"), (t + cyc / 2, 100, "inout"), (t + cyc, 0, "inout")]
        t += cyc
    yield Layer([group([ellipse(ctx.px(20)), fill(color)])], name="dot", pos=Anim(pk), opacity=Anim(ok), start=t0)


def polyline_layer(pts, color, width, draw, start):
    return Layer([group([path(pts), trim(0, draw), stroke(color, width)], name="path")], name="path", start=start)


def build(doc, sc, ctx):
    beat, dur, H, W = ctx.beat or 0.5, sc["dur"], ctx.H, ctx.W
    tb = sc.get("topBeat", 1) * beat
    bb = sc.get("bottomBeat", round(dur / beat * 0.3)) * beat
    ab = sc.get("arrowBeat", round(dur / beat * 0.62)) * beat
    top, bot = sc["top"], sc["bottom"]
    acc, acc2 = ctx.th("accent"), ctx.th("accent2")
    layers = []
    layers += atom(ctx, W / 2, H * 0.15, acc, tb, dur)
    layers += heading(ctx, top["title"], H * 0.255, acc, tb + 0.25)
    layers += tag_row(ctx, top["items"], H * 0.318, tb + 0.5, dur)
    layers += globe(ctx, W / 2, H * 0.545, acc2, bb, dur)
    layers += heading(ctx, bot["title"], H * 0.65, acc2, bb + 0.25)
    layers += tag_row(ctx, bot["items"], H * 0.747, bb + 0.5, dur)
    y1, y2 = H * 0.35, H * 0.475
    layers += arrow(ctx, W * 0.2, y1, y2, acc, ab, dur)
    layers += arrow(ctx, W * 0.8, y2, y1, acc2, ab + 0.18, dur)
    ym = (y1 + y2) / 2
    label = sc.get("bridge", "useEffect")
    size = ctx.px(62)
    tl = mono_text(label, size, x=0, y=0, color=ctx.th("onAccent"), preset="none")
    w, h = measure(label, size, "jetbrains-mono", 0.0) + ctx.px(130), ctx.px(128)
    t = ab + 0.35
    layers.append(Layer(tl.shapes + [group([rect((w, h), (0, 0), h / 2), fill(ctx.th("fg"))], name="pill")], name="bridge",
                        pos=Anim([(t, (W / 2, ym), "out")] + idle_keys((W / 2, ym), t, dur, 0, ctx.px(5), 3.4)[1:]),
                        scale=Anim(spring_scale(t, 0.55, 60, 112)), opacity=Anim([(t, 0, "out"), (t + 0.2, 100)]), start=t))
    pk, ok, tt = [], [], t + 0.15
    while tt < dur:
        pk += [(tt, (100, 100), "out"), (tt + 1.8, (135, 170), "out")]
        ok += [(tt, 70, "out"), (tt + 1.8, 0, "out")]
        tt += 1.8
    layers.append(Layer([group([rect((w, h), (0, 0), h / 2), stroke(ctx.th("fg"), ctx.px(4))], name="ring")], name="pulse", pos=(W / 2, ym), scale=Anim(pk), opacity=Anim(ok), start=t + 0.15))
    doc.add(layers)
