"""pipeline: a vertical timeline. Cards slide in, the node of the current step lights up and pulses,
and the rail draws itself to the next one.

  { "type": "custom", "file": "lottie/scenes/pipeline.py", "dur": 10, "title": "What happens on a state change",
    "steps": [ { "title": "Render", "sub": "React calls your component" }, ... ],
    "stepBeats": [1, 4, 7, 10] }
"""
from rs import *


def build(doc, sc, ctx):
    beat, dur, W, H = ctx.beat or 0.5, sc["dur"], ctx.W, ctx.H
    steps = sc["steps"]
    n = len(steps)
    length = dur / beat
    sb = sc.get("stepBeats") or [1 + round(q * (length - 2) / n) for q in range(n)]
    at = [b * beat for b in sb]
    if sc.get("title"):
        size = fit_size(ctx, [sc["title"]], ctx.px(70), ctx.W * 0.86, cap=ctx.px(76))
        ty = H * 0.115
        doc.add(rise_layer(text(sc["title"], size=size, x=W / 2, y=ty, color=ctx.th("fg"), preset="none", name="title"), 0.0, 0.6, ctx.px(50)))
        doc.add(underline(ctx, W / 2, ty + size * 0.36 + ctx.px(26), min(measure(sc["title"], size) * 0.35, W * 0.3), ctx.th("accent2"), 0.3, 0.5))
    rail_x, y0, gap = W * 0.14, H * 0.27, H * 0.152
    card_l = rail_x + ctx.px(70)
    card_w, card_h = W * 0.95 - card_l, ctx.px(210)
    # the dim rail the progress runs along
    doc.add(Layer([group([path([(rail_x, y0), (rail_x, y0 + (n - 1) * gap)]), trim(0, Anim([(0.2, 0, "out"), (1.4, 100)])), stroke(ctx.th("muted"), ctx.px(6), opacity=35)])], name="rail", start=0.2))
    for i in range(1, n):  # progress along the rail first, so the nodes sit on top of it
        a, y = at[i], y0 + i * gap
        doc.add(Layer([group([path([(rail_x, y0 + (i - 1) * gap), (rail_x, y)]), trim(0, Anim([(a - 0.4, 0, "out"), (a, 100)])), stroke(ctx.th("accent"), ctx.px(9))])], name="progress", start=a - 0.4))
    for i, st in enumerate(steps):
        y = y0 + i * gap
        a = at[i]
        nxt = at[i + 1] if i + 1 < n else None
        hot = [(a, 0, "out"), (a + 0.25, 100, "linear")] + ([(nxt - 0.1, 100, "out"), (nxt + 0.25, 0)] if nxt else [])
        # card
        title = text(st["title"], size=ctx.px(82), x=0, y=0, color=ctx.th("fg"), preset="none", align="left")
        title_hot = text(st["title"], size=ctx.px(82), x=0, y=0, color=ctx.th("accent"), preset="none", align="left")
        sub = text(st.get("sub", ""), size=fit_size(ctx, [st.get("sub", " ")], ctx.px(46), card_w - ctx.px(110)), x=0, y=0, color=ctx.th("muted"), preset="none", align="left")
        cx = card_l + card_w / 2
        left = -card_w / 2 + ctx.px(70)
        shapes = [group(title_hot.shapes, pos=(left, -ctx.px(34)), opacity=Anim(hot), name="title-hot"),
                  group(title.shapes, pos=(left, -ctx.px(34)), name="title"), group(sub.shapes, pos=(left, ctx.px(50)), name="sub"),
                  group([rect((ctx.px(10), card_h * 0.62), (0, 0), ctx.px(5)), fill(ctx.th("accent"))], pos=(-card_w / 2 + ctx.px(26), 0), opacity=Anim(hot), name="bar"),
                  group([rect((card_w, card_h), (0, 0), ctx.px(34)), fill(ctx.th("panel"))], name="card")]
        doc.add(Layer(shapes, name=f"step-{i}", pos=Anim([(a, (cx + ctx.px(90), y), "out"), (a + 0.6, (cx, y), "out")]), scale=Anim(spring_scale(a, 0.6, 94, 103)),
                      opacity=Anim([(a, 0, "out"), (a + 0.3, 100)]), start=a))
        # node on the rail
        num = text(str(i + 1), size=ctx.px(54), x=0, y=0, color=ctx.th("accent"), preset="none")
        num_hot = text(str(i + 1), size=ctx.px(54), x=0, y=0, color=ctx.th("onAccent"), preset="none")
        r = ctx.px(46)
        node = [group(num_hot.shapes, opacity=Anim(hot), name="num-hot"), group(num.shapes, name="num"),
                group([ellipse(r * 2), fill(ctx.th("accent"))], opacity=Anim(hot), name="fill"),
                group([ellipse(r * 2), trim(0, Anim([(a - 0.15, 0, "out"), (a + 0.25, 100)])), stroke(ctx.th("accent"), ctx.px(7))], name="ring"),
                group([ellipse(r * 2), fill(ctx.th("bg2", ctx.th("panel")))], name="disc")]
        doc.add(Layer(node, name=f"node-{i}", pos=(rail_x, y), scale=Anim(spring_scale(a - 0.15, 0.5, 0, 120)), start=a - 0.15))
        # pulse while this step is current
        end = nxt if nxt else dur
        pk, ok, t = [], [], a
        while t < end:
            pk += [(t, (100, 100), "out"), (t + 1.3, (220, 220), "out")]
            ok += [(t, 75, "out"), (t + 1.3, 0, "out")]
            t += 1.3
        doc.add(Layer([group([ellipse(r * 2), stroke(ctx.th("accent"), ctx.px(5))])], name=f"pulse-{i}", pos=(rail_x, y), scale=Anim(pk), opacity=Anim(ok), start=a, end=end))
