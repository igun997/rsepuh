"""chips: pills that slide in with a spring, tilt into place and float (spec: items, title, sub, itemT)."""
from rs import *


def build(doc, sc, ctx):
    items = sc.get("items", [])
    n, dur = len(items), sc["dur"]
    gap = ctx.px(188)
    y0 = ctx.cy - (n - 1) * gap / 2 + (ctx.px(70) if sc.get("title") else 0) - (ctx.px(30) if sc.get("sub") else 0)
    if sc.get("title"):
        title = ctx.caps(sc, sc["title"])
        size = fit_size(ctx, [title], ctx.px(92), ctx.W * 0.86, cap=ctx.px(100), tracking=tracking(ctx))
        ty = y0 - ctx.px(230)
        doc.add(rise_layer(text(title, size=size, x=ctx.W / 2, y=ty, color=ctx.th("fg"), preset="none", tracking=tracking(ctx), name="title"), 0.0, 0.6, ctx.px(60)))
        doc.add(underline(ctx, ctx.W / 2, ty + size * 0.36 + ctx.px(30), min(measure(title, size) * 0.4, ctx.W * 0.3), ctx.th("accent2"), 0.3, 0.5))
    cols = [ctx.th("accent"), ctx.th("accent2"), ctx.th("fg"), ctx.th("accent")]
    itemT = sc.get("itemT") or [0.25 + k * ctx.th("stagger", 0.12) * 2.2 for k in range(n)]
    labels = [ctx.caps(sc, i) for i in items]
    size = fit_size(ctx, labels, ctx.px(84), ctx.W * 0.74, cap=ctx.px(92))
    for k, label in enumerate(labels):
        side = -1 if k % 2 == 0 else 1
        layer, _, _ = chip(ctx, label, ctx.W / 2, y0 + k * gap, cols[k % len(cols)], ctx.th("onAccent"), size, itemT[k], dur, dur, side, k, tilt=side * ctx.th("tilt", 0) * 0.5, name=f"chip-{k}")
        doc.add(layer)
    if sc.get("sub"):
        s = text(sc["sub"], size=fit_size(ctx, [sc["sub"]], ctx.px(52), ctx.W * 0.8), x=ctx.W / 2, y=y0 + (n - 1) * gap + ctx.px(190), color=ctx.th("muted"), preset="none", name="sub")
        doc.add(rise_layer(s, (itemT[-1] + 0.5) if n else 0.9, 0.5, ctx.px(40)))
