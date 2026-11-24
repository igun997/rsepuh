"""Helpers shared by the built-in scene writers (and by your own in lottie/scenes/*.py).

A scene writer is a module with `build(doc, sc, ctx)`: it adds layers to `doc` (a kit.Doc the size of
the video, transparent, as long as the scene) using the scene dict `sc` from the spec and the style in
`ctx`. Times inside the scene are seconds from the scene start. Sizes are written for a 1080 px short
side: wrap them in ctx.px(...) so the scene scales with the frame.
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from kit import *  # noqa: F401,F403,E402


class Ctx:
    def __init__(self, plan):
        spec = plan["spec"]
        self.plan, self.theme, self.spec = plan, plan["theme"], spec
        self.W, self.H = spec["size"]
        self.fps = spec["fps"]
        self.S = min(self.W, self.H) / 1080.0
        self.beat = spec.get("beat")
        self.name = spec["name"]

    def px(self, v):
        return v * self.S

    def th(self, key, default=None):
        return self.theme.get(key, default)

    @property
    def cy(self):
        """Optical centre of the content zone: subtitles live below 0.8 of the height."""
        return self.H * 0.44

    def caps(self, sc, s):
        up = sc.get("caps", self.theme.get("caps"))
        return s.upper() if up else s

    def preset(self):
        """Per-letter entrance that matches the style: springy drops for `pop` styles, rises for calm ones."""
        return "drop" if self.theme.get("motion", "pop") == "pop" else "rise"


def fit_size(ctx, strings, size, max_w, fontname="inter-bold", tracking=-0.01, cap=None):
    """Largest font size <= size (px, already scaled) at which every string fits in max_w."""
    s = size
    for t in strings:
        w = measure(t, size, fontname, tracking)
        if w > max_w:
            s = min(s, size * max_w / w)
    return min(s, cap) if cap else s


def underline(ctx, x, y, w, color, t, d=0.5, end=None):
    h = ctx.px(9)
    shape = group([rect((w, h), (0, 0), h / 2), fill(color)], scale=Anim([(t, (0, 100), "out"), (t + d, (100, 100))]), name="underline")
    return Layer([shape], name="underline", pos=(x, y), start=t, end=end)


def chip(ctx, label, x, y, bg, fg, size, at, end, seconds, side=1, k=0, pad_x=150, height=156, tilt=0.0, float_amp=7.0, name="chip"):
    """A pill with a label inside that slides in from a side with a spring, tilts into place and floats."""
    tl = text(label, size=size, x=0, y=0, color=fg, preset="none")
    w = measure(label, size) + ctx.px(pad_x)
    h = ctx.px(height)
    shapes = tl.shapes + [group([rect((w, h), (0, 0), h / 2), fill(bg)], name="pill")]
    slide = [(at, (x + side * ctx.px(200), y), "out"), (at + 0.55, (x, y), "out")]
    pos = Anim(slide + idle_keys((x, y), at + 0.55, seconds, 0, ctx.px(float_amp), 3.6 + k * 0.4, k * 1.3)[1:])
    return Layer(shapes, name=name, pos=pos, scale=Anim(spring_scale(at, 0.55, 80, 110)), rot=Anim([(at, side * 6, "out"), (at + 0.6, tilt)]),
                 opacity=Anim([(at, 0, "out"), (at + 0.25, 100)]), start=at, end=end), w, h


def rise_layer(layer, at, d=0.6, dist=60, ctx=None):
    """Whole-layer rise + fade in (for blocks that are not per-letter)."""
    x, y = layer.pos
    layer.pos = Anim([(at, (x, y + dist), "out"), (at + d, (x, y))])
    layer.opacity = Anim([(at, 0, "out"), (at + d * 0.55, 100)])
    layer.start = at
    return layer


def as_group(layer, pos=(0, 0), name="inner"):
    """Reuse the shapes of a text() layer inside another layer, offset by `pos`."""
    return group(layer.shapes, pos=pos, name=name)


def tracking(ctx):
    """Style tracking (AE units, 1/1000 em) as the em fraction the kit uses."""
    return ctx.theme.get("tracking", 0) / 1000.0


def panel_rect(ctx, w, h, color, radius, ring=None, ring_w=3, ring_opacity=100):
    items = [rect((w, h), (0, 0), radius), fill(color)]
    if ring:
        items.insert(1, stroke(ring, ctx.px(ring_w), opacity=ring_opacity))
    return group(items, name="panel")
