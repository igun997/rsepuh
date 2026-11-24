"""hook: one big word at a time, each landing on its beat (spec: words | text, sub, wordT, colors)."""
from rs import *


def build(doc, sc, ctx):
    words = sc.get("words") or str(sc.get("text", "")).split()
    n, dur = len(words), sc["dur"]
    times = sc.get("wordT") or [k * (dur * 0.78 / n if n > 1 else dur) for k in range(n)]
    for k, w in enumerate(words):
        a = times[k]
        b = times[k + 1] if k + 1 < n else dur
        t = ctx.caps(sc, w)
        size = fit_size(ctx, [t], ctx.px(250), ctx.W * 0.86, cap=ctx.px(320))
        col = (sc.get("colors") or [None] * n)[k] or (ctx.th("fg") if k % 2 == 0 else ctx.th("accent"))
        layer = text(t, size=size, x=ctx.W / 2, y=ctx.cy, color=col, preset=ctx.preset(), start=a, stagger=0.02, tracking=tracking(ctx), name=f"word-{k}")
        layer.start, layer.end = a, b
        if k + 1 < n:
            layer.opacity = Anim([(b - 0.1, 100, "inout"), (b, 0)])
        doc.add(layer)
        cap = size * 0.73
        uw = min(measure(t, size, tracking=tracking(ctx)) * 0.5, ctx.W * 0.4)
        doc.add(underline(ctx, ctx.W / 2, ctx.cy + cap / 2 + ctx.px(46), uw, ctx.th("accent") if k % 2 == 0 else ctx.th("accent2"), a + 0.12, 0.45, end=b))
    if sc.get("sub"):
        s = text(sc["sub"], size=fit_size(ctx, [sc["sub"]], ctx.px(56), ctx.W * 0.8), x=ctx.W / 2, y=ctx.cy + ctx.px(300), color=ctx.th("muted"), preset="none", name="sub")
        doc.add(rise_layer(s, dur * 0.55, 0.5, ctx.px(46)))
