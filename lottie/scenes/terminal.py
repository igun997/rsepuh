"""terminal: a window where lines starting with $ are typed key by key (spec: lines, cps)."""
from rs import *


def build(doc, sc, ctx):
    lines = [str(l) for l in sc.get("lines", [])]
    n = len(lines)
    w, h = ctx.W * 0.92, max(ctx.px(560), ctx.px(190) + n * ctx.px(124))
    cx, cy = ctx.W / 2, ctx.cy
    fs = ctx.px(50)
    shapes = [panel_rect(ctx, w, h, ctx.th("panel"), ctx.px(36), ring=ctx.th("accent"), ring_w=3, ring_opacity=45)]
    for d, c in enumerate(["#ff5f57", "#febc2e", "#28c840"]):
        shapes.insert(0, group([ellipse(ctx.px(30)), fill(c)], pos=(-w / 2 + ctx.px(62 + 48 * d), -h / 2 + ctx.px(58)), name="dot"))
    doc.add(Layer(shapes, name="window", pos=(cx, cy), scale=Anim(spring_scale(0.0, 0.5, 92, 103)),
                  opacity=Anim([(0.0, 0, "out"), (0.25, 100)])))
    cps = sc.get("cps", 26)
    t, top, left = 0.55, cy - h / 2 + ctx.px(160), cx - w / 2 + ctx.px(64)
    last_end = None
    for k, s in enumerate(lines):
        prompt = s.startswith("$")
        step = (1.0 / cps) if prompt else (0.25 / max(len(s), 1))
        doc.add(mono_text(s, fs, x=left, y=top + k * ctx.px(118), align="left", color=ctx.th("accent") if prompt else ctx.th("fg"), preset="type", start=t, stagger=step, name=f"line-{k}"))
        end = t + len(s) * step
        if prompt:
            last_end = (k, end, left + len(s) * fs * 0.6)
        t = end + (0.35 if prompt else 0.15)
    if last_end:
        k, t0, x = last_end
        keys, tt, on = [], t0, True
        while tt < sc["dur"]:
            keys.append((tt, 100 if on else 0, "hold"))
            tt += 0.5
            on = not on
        keys.append((sc["dur"], 0))
        doc.add(Layer([group([rect((fs * 0.5, fs * 0.9), (0, 0)), fill(ctx.th("accent"))])], name="cursor", pos=(x + fs * 0.35, top + k * ctx.px(118)), opacity=Anim(keys), start=t0))
