"""stat: a big number counting up with an ease-out (spec: value, from, prefix, suffix, label, countDur)."""
from rs import *


def build(doc, sc, ctx):
    a, b = sc.get("from", 0), sc["value"]
    pre, suf = sc.get("prefix", ""), sc.get("suffix", "")
    cd = sc.get("countDur") or min(1.2, sc["dur"] * 0.6)
    full = f"{pre}{b}{suf}"
    size = fit_size(ctx, [full], ctx.px(420), ctx.W * 0.86, cap=ctx.px(460))
    fps, frames = ctx.fps, int(round(cd * ctx.fps))
    states, last = [], None
    for f in range(frames + 1):
        p = min(1.0, f / max(frames, 1))
        v = round(a + (b - a) * (1 - (1 - p) ** 3))
        if v != last:
            states.append([v, f / fps])
            last = v
    for i, (v, t) in enumerate(states):
        end = states[i + 1][1] if i + 1 < len(states) else sc["dur"]
        layer = text(f"{pre}{v}{suf}", size=size, x=ctx.W / 2, y=ctx.cy, color=ctx.th("accent"), preset="none", name=f"n-{v}")
        layer.start, layer.end = t, end
        if i == 0:
            layer.scale = Anim(spring_scale(0.0, 0.5, 70, 110))
            layer.pos = (ctx.W / 2, ctx.cy)
        doc.add(layer)
    if sc.get("label"):
        lab = text(ctx.caps(sc, sc["label"]), size=fit_size(ctx, [sc["label"]], ctx.px(70), ctx.W * 0.84), x=ctx.W / 2, y=ctx.cy + ctx.px(330), color=ctx.th("fg"), preset="none", name="label")
        doc.add(rise_layer(lab, 0.3, 0.5, ctx.px(44)))
