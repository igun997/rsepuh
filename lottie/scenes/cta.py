"""cta: end card: title, lines, a pulsing button and a url (spec: title, lines, button, url, buttonT, urlT)."""
import math
from rs import *


def build(doc, sc, ctx):
    ty = ctx.cy - ctx.px(90)
    title = sc.get("title", "")
    t = title if sc.get("caps") is False else ctx.caps(sc, title)
    size = fit_size(ctx, [t], ctx.px(230), ctx.W * 0.88, cap=ctx.px(250), tracking=tracking(ctx))
    doc.add(text(t, size=size, x=ctx.W / 2, y=ty, color=ctx.th("fg"), preset=ctx.preset(), start=0.0, stagger=0.03, tracking=tracking(ctx), name="title"))
    for k, line in enumerate(sc.get("lines") or ([sc["sub"]] if sc.get("sub") else [])):
        s = text(line, size=fit_size(ctx, [line], ctx.px(60), ctx.W * 0.8), x=ctx.W / 2, y=ty + ctx.px(170 + k * 80), color=ctx.th("muted"), preset="none", name=f"line-{k}")
        doc.add(rise_layer(s, 0.3 + k * 0.1, 0.55, ctx.px(44)))
    if sc.get("button"):
        by = ctx.cy + ctx.px(270)
        at = sc.get("buttonT", 0.5)
        label = ctx.caps(sc, sc["button"])
        size_b = fit_size(ctx, [label], ctx.px(78), ctx.W * 0.6, cap=ctx.px(86))
        tl = text(label, size=size_b, x=0, y=0, color=ctx.th("onAccent"), preset="none")
        w = measure(label, size_b) + ctx.px(150)
        h = ctx.px(176)
        pulse = [(at + 0.6 + i / 15.0, (100 + 3.5 * math.sin((i / 15.0) * 6),) * 2, "inout") for i in range(0, int((sc["dur"] - at - 0.6) * 15) + 1)]
        scale = Anim(spring_scale(at, 0.6, 70, 112) + pulse[1:])
        doc.add(Layer(tl.shapes + [group([rect((w, h), (0, 0), h / 2), fill(ctx.th("accent"))], name="pill")], name="button", pos=(ctx.W / 2, by), scale=scale,
                      opacity=Anim([(at, 0, "out"), (at + 0.2, 100)]), start=at))
    if sc.get("url"):
        u = mono_text(sc["url"], fit_size(ctx, [sc["url"]], ctx.px(56), ctx.W * 0.84, fontname="jetbrains-mono", tracking=0.0), x=ctx.W / 2, y=ctx.cy + ctx.px(520), color=ctx.th("accent"), preset="none", name="url")
        doc.add(rise_layer(u, sc.get("urlT", 0.7), 0.5, ctx.px(40)))
