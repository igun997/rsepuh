"""title: stacked lines rising in one after another, the last in the accent colour (spec: lines | text, sub)."""
from rs import *


def build(doc, sc, ctx):
    lines = [ctx.caps(sc, l) for l in (sc.get("lines") or [sc.get("text", "")])]
    n = len(lines)
    size = fit_size(ctx, lines, ctx.px(160), ctx.W * 0.86, cap=ctx.px(190), tracking=tracking(ctx))
    line_h = size * 1.16
    y0 = ctx.cy - (n - 1) * line_h / 2 - (ctx.px(40) if sc.get("sub") else 0)
    for k, l in enumerate(lines):
        col = ctx.th("accent") if (k == n - 1 and n > 1) else ctx.th("fg")
        a = 0.05 + k * 0.14
        doc.add(text(l, size=size, x=ctx.W / 2, y=y0 + k * line_h, color=col, preset="rise", start=a, stagger=0.02, tracking=tracking(ctx), name=f"line-{k}"))
    last = lines[-1]
    cap = size * 0.73
    doc.add(underline(ctx, ctx.W / 2, y0 + (n - 1) * line_h + cap / 2 + ctx.px(34), min(measure(last, size, tracking=tracking(ctx)) * 0.6, ctx.W * 0.5), ctx.th("accent2"), 0.05 + (n - 1) * 0.14 + 0.4, 0.55))
    if sc.get("sub"):
        s = text(sc["sub"], size=fit_size(ctx, [sc["sub"]], ctx.px(54), ctx.W * 0.8), x=ctx.W / 2, y=y0 + n * line_h + ctx.px(40), color=ctx.th("muted"), preset="none", name="sub")
        doc.add(rise_layer(s, 0.5, 0.5, ctx.px(40)))
