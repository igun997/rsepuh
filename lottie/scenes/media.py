"""media: the picture / 3D overlay are footage (handled when compositing); this adds the caption pill."""
from rs import *


def build(doc, sc, ctx):
    if not sc.get("caption"):
        return
    label = ctx.caps(sc, sc["caption"])
    size = fit_size(ctx, [label], ctx.px(74), ctx.W * 0.84, cap=ctx.px(84))
    at = sc.get("captionT", 0.3)
    tl = text(label, size=size, x=0, y=0, color=ctx.th("onAccent"), preset="none")
    w, h = measure(label, size) + ctx.px(100), size * 0.73 + ctx.px(64)
    cy = ctx.H * sc.get("captionY", 0.7)
    doc.add(Layer(tl.shapes + [group([rect((w, h), (0, 0), ctx.px(30)), fill(ctx.th("accent"))], name="pill")], name="caption",
                  pos=Anim([(at, (ctx.W / 2, cy + ctx.px(40)), "out"), (at + 0.5, (ctx.W / 2, cy))]), scale=Anim(spring_scale(at, 0.45, 80, 108)),
                  opacity=Anim([(at, 0, "out"), (at + 0.2, 100)]), start=at))
