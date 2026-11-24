"""rsepuh Lottie kit: author Lottie (Bodymovin) motion in Python, text included.

    import sys; sys.path.insert(0, "lottie")
    from kit import *

    d = Doc("hello", 1080, 600, fps=30, seconds=3)
    d.add(text("Edit like a [[veteran]]", font="inter-bold", size=150, color="#e6edf3", accent="#61dafb",
               x=540, y=300, preset="rise", start=0, stagger=1.5))
    d.save("assets/lottie/hello.json")            # flat file EffectCraft imports (transparent, no slots)
    d.save_player("player/public/projects/main/scene-1/lottie.json")   # same motion with slots for the Skottie player

Why a kit: text is drawn as vector glyph shapes (outlines in lottie/fonts), so it renders the same in
the Skottie player and in EffectCraft with no font installed, and every letter can move on its own.
Everything is deterministic: same script, same JSON. Times are in seconds unless a name ends in `_f`.
"""
import json
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))

# --------------------------------------------------------------------------- easing
# cubic-bezier handles (x1, y1, x2, y2). y beyond 0..1 overshoots, which is what makes a spring.
EASE = {
    "linear": (0.0, 0.0, 1.0, 1.0),
    "out": (0.22, 1.0, 0.36, 1.0),         # fast start, long gentle landing (out-expo like)
    "out_soft": (0.33, 1.0, 0.68, 1.0),
    "in": (0.64, 0.0, 0.78, 0.0),
    "inout": (0.65, 0.0, 0.35, 1.0),
    "spring": (0.34, 1.56, 0.64, 1.0),     # overshoots once and settles
    "spring_hard": (0.3, 2.1, 0.55, 1.0),
    "hold": None,
}


def color(c):
    """'#rrggbb' or (r, g, b) in 0..1 or 0..255 -> [r, g, b, 1]."""
    if isinstance(c, str):
        c = c.lstrip("#")
        if len(c) == 3:
            c = "".join(ch * 2 for ch in c)
        return [int(c[i:i + 2], 16) / 255 for i in (0, 2, 4)] + [1]
    r, g, b = c[:3]
    if max(r, g, b) > 1:
        r, g, b = r / 255, g / 255, b / 255
    return [r, g, b, 1]


class Anim:
    """Keyframes: Anim([(t, value, ease_to_next), ...]); t in seconds (kit converts to frames)."""

    def __init__(self, keys):
        self.keys = keys


def _vec(v):
    return list(v) if isinstance(v, (list, tuple)) else [v]


def prop(v, fps, is_color=False):
    """A Lottie property from a constant or an Anim."""
    if isinstance(v, Anim):
        ks = []
        for n, k in enumerate(v.keys):
            t, val = k[0], k[1]
            ease = k[2] if len(k) > 2 else "out"
            kf = {"t": round(t * fps, 3), "s": _vec(val)}
            nxt = v.keys[n + 1] if n + 1 < len(v.keys) else None
            if nxt is not None:
                h = EASE[ease]
                if h is None:
                    kf["h"] = 1
                else:
                    kf["o"] = {"x": [h[0]], "y": [h[1]]}
                    kf["i"] = {"x": [h[2]], "y": [h[3]]}
            ks.append(kf)
        return {"a": 1, "k": ks}
    if is_color:
        return {"a": 0, "k": color(v)}
    return {"a": 0, "k": _vec(v) if isinstance(v, (list, tuple)) else v}


# --------------------------------------------------------------------------- shapes
class Ctx:
    fps = 30


def _tr(pos=(0, 0), anchor=(0, 0), scale=(100, 100), rot=0, opacity=100):
    f = Ctx.fps
    return {"ty": "tr", "p": prop(list(pos) if not isinstance(pos, Anim) else pos, f), "a": prop(anchor if not isinstance(anchor, tuple) else list(anchor), f),
            "s": prop(scale if isinstance(scale, Anim) else list(scale), f), "r": prop(rot, f), "o": prop(opacity, f), "sk": prop(0, f), "sa": prop(0, f)}


def ellipse(size, pos=(0, 0)):
    s = size if isinstance(size, (list, tuple, Anim)) else (size, size)
    return {"ty": "el", "p": prop(pos if isinstance(pos, Anim) else list(pos), Ctx.fps), "s": prop(s if isinstance(s, Anim) else list(s), Ctx.fps)}


def rect(size, pos=(0, 0), radius=0):
    return {"ty": "rc", "p": prop(pos if isinstance(pos, Anim) else list(pos), Ctx.fps), "s": prop(size if isinstance(size, Anim) else list(size), Ctx.fps),
            "r": prop(radius, Ctx.fps)}


def path(vertices, closed=False, smooth=0.0):
    """Polyline through points; smooth 0..0.5 rounds the corners with Bezier handles."""
    n = len(vertices)
    v = [list(p) for p in vertices]
    i, o = [[0, 0] for _ in v], [[0, 0] for _ in v]
    if smooth:
        for k in range(n):
            if not closed and k in (0, n - 1):
                continue
            a, b, c = v[k - 1], v[k], v[(k + 1) % n]
            i[k] = [(a[0] - b[0]) * smooth, (a[1] - b[1]) * smooth]
            o[k] = [(c[0] - b[0]) * smooth, (c[1] - b[1]) * smooth]
    return {"ty": "sh", "ks": {"a": 0, "k": {"i": i, "o": o, "v": v, "c": bool(closed)}}}


def bezier_path(v, i, o, closed=True):
    return {"ty": "sh", "ks": {"a": 0, "k": {"i": i, "o": o, "v": v, "c": closed}}}


def star(points, outer, inner, rot=0):
    v = []
    for k in range(points * 2):
        r = outer if k % 2 == 0 else inner
        a = math.radians(rot - 90 + k * 180 / points)
        v.append([r * math.cos(a), r * math.sin(a)])
    return path(v, closed=True)


def fill(c, opacity=100):
    return {"ty": "fl", "c": prop(c, Ctx.fps, True), "o": prop(opacity, Ctx.fps), "r": 1}


def stroke(c, width, opacity=100, cap="round", join="round"):
    caps = {"butt": 1, "round": 2, "square": 3}
    joins = {"miter": 1, "round": 2, "bevel": 3}
    return {"ty": "st", "c": prop(c, Ctx.fps, True), "o": prop(opacity, Ctx.fps), "w": prop(width, Ctx.fps), "lc": caps[cap], "lj": joins[join], "ml": 4}


def trim(start=0, end=100, offset=0):
    return {"ty": "tm", "s": prop(start, Ctx.fps), "e": prop(end, Ctx.fps), "o": prop(offset, Ctx.fps), "m": 1}


def group(items, pos=(0, 0), anchor=(0, 0), scale=(100, 100), rot=0, opacity=100, name="group"):
    return {"ty": "gr", "nm": name, "it": list(items) + [_tr(pos, anchor, scale, rot, opacity)]}


# --------------------------------------------------------------------------- layers and document
class Layer:
    def __init__(self, shapes, name="layer", pos=(0, 0), anchor=(0, 0), scale=(100, 100), rot=0, opacity=100, start=0.0, end=None):
        self.shapes, self.name = shapes, name
        self.pos, self.anchor, self.scale, self.rot, self.opacity = pos, anchor, scale, rot, opacity
        self.start, self.end = start, end


class Doc:
    def __init__(self, name, w, h, fps=30, seconds=3.0, background=None):
        self.name, self.w, self.h, self.fps, self.seconds = name, w, h, fps, seconds
        self.layers = []
        self.background = background  # colour for the player preview only; flat files are transparent
        Ctx.fps = fps

    def add(self, layer):
        """Layers added later sit on top."""
        if isinstance(layer, list):
            for l in layer:
                self.layers.append(l)
        else:
            self.layers.append(layer)
        return self

    def _layer_json(self, l, ind, slots):
        f = self.fps
        end = l.end if l.end is not None else self.seconds
        def v3(v, z):
            if isinstance(v, Anim):
                return Anim([(k[0], tuple(k[1]) + (z,), *k[2:]) for k in v.keys])
            return list(v) + [z]

        ks = {"o": prop(l.opacity, f), "r": prop(l.rot, f), "p": prop(v3(l.pos, 0), f), "a": prop(v3(l.anchor, 0), f), "s": prop(v3(l.scale, 100), f)}
        return {"ddd": 0, "ind": ind, "ty": 4, "nm": l.name, "sr": 1, "ks": ks, "ao": 0, "shapes": l.shapes,
                "ip": round(l.start * f, 3), "op": round(end * f, 3), "st": 0, "bm": 0}

    def to_json(self, player=False):
        Ctx.fps = self.fps
        layers = list(self.layers)
        slots = {}
        if player:
            slots = {"bgColor": {"p": {"a": 0, "k": color(self.background or "#0d1117")}}, "bgOpacity": {"p": {"a": 0, "k": 100}}}
            bg = Layer([group([rect((self.w, self.h), (0, 0)), {"ty": "fl", "c": {"sid": "bgColor"}, "o": {"sid": "bgOpacity"}, "r": 1}], name="background")],
                       name="background", pos=(self.w / 2, self.h / 2))
            layers = [bg] + layers
        # Lottie lists the top layer first
        out = [self._layer_json(l, n + 1, slots) for n, l in enumerate(reversed(layers))]
        doc = {"v": "5.7.0", "fr": self.fps, "ip": 0, "op": round(self.seconds * self.fps, 3), "w": self.w, "h": self.h, "nm": self.name, "ddd": 0, "assets": [], "layers": out}
        if slots:
            doc["slots"] = slots
        return doc

    def save(self, path_):
        """Flat file for EffectCraft: transparent, no slots."""
        os.makedirs(os.path.dirname(os.path.abspath(path_)), exist_ok=True)
        with open(path_, "w") as fh:
            json.dump(self.to_json(False), fh, separators=(",", ":"))
        return path_

    def save_player(self, path_, controls=True):
        """Skottie player variant: a background layer wired to slots (bgColor, bgOpacity) plus controls.json."""
        os.makedirs(os.path.dirname(os.path.abspath(path_)), exist_ok=True)
        with open(path_, "w") as fh:
            json.dump(self.to_json(True), fh, indent=1)
        if controls:
            with open(os.path.join(os.path.dirname(path_), "controls.json"), "w") as fh:
                json.dump({"controls": [{"sid": "bgColor", "label": "Background color"},
                                        {"sid": "bgOpacity", "label": "Background opacity", "min": 0, "max": 100, "step": 1}]}, fh, indent=1)
        return path_


# --------------------------------------------------------------------------- vector text
_FONTS = {}


def font(name):
    if name not in _FONTS:
        with open(os.path.join(HERE, "fonts", name + ".json")) as fh:
            _FONTS[name] = json.load(fh)
    return _FONTS[name]


def measure(text_, size, fontname="inter-bold", tracking=0.0):
    fd = font(fontname)
    k = size / fd["upm"]
    w = 0.0
    for ch in text_:
        g = fd["glyphs"].get(ch) or fd["glyphs"].get("?")
        w += g["adv"] * k + tracking * size
    return w - tracking * size


def _glyph_shapes(fd, ch, k, c):
    g = fd["glyphs"].get(ch) or fd["glyphs"].get("?")
    paths = []
    for p in g["paths"]:
        paths.append(bezier_path([[x * k, y * k] for x, y in p["v"]], [[x * k, y * k] for x, y in p["i"]], [[x * k, y * k] for x, y in p["o"]], True))
    return paths + [fill(c)], g["adv"] * k


# per-glyph motion presets: (index, count, t0, stagger) -> dict of animated props
def _rise(i, n, t0, st, d=0.7, dist=0.9):
    t = t0 + i * st
    return dict(dy=Anim([(t, dist, "out"), (t + d, 0)]), opacity=Anim([(t, 0, "out"), (t + d * 0.45, 100)]), scale=None, rot=None)


def _pop(i, n, t0, st, d=0.6, **_):
    t = t0 + i * st
    return dict(dy=None, opacity=Anim([(t, 0, "linear"), (t + 0.05, 100)]),
                scale=Anim([(t, (40, 40), "spring"), (t + d, (100, 100))]), rot=None)


def _drop(i, n, t0, st, d=0.65, **_):
    t = t0 + i * st
    return dict(dy=Anim([(t, -1.1, "spring"), (t + d, 0)]), opacity=Anim([(t, 0, "linear"), (t + 0.05, 100)]),
                scale=None, rot=Anim([(t, -14 if i % 2 else 14, "spring"), (t + d, 0)]))


def _flip(i, n, t0, st, d=0.6, **_):
    t = t0 + i * st
    return dict(dy=None, opacity=Anim([(t, 0, "linear"), (t + 0.06, 100)]),
                scale=Anim([(t, (100, 0), "spring"), (t + d, (100, 100))]), rot=None)


def _fade(i, n, t0, st, d=0.5, **_):
    t = t0 + i * st
    return dict(dy=None, opacity=Anim([(t, 0, "inout"), (t + d, 100)]), scale=Anim([(t, (92, 92), "out"), (t + d, (100, 100))]), rot=None)


def _type(i, n, t0, st, **_):
    t = t0 + i * st
    return dict(dy=None, opacity=Anim([(t, 0, "hold"), (t + 0.001, 100)]), scale=None, rot=None)


PRESETS = {"type": _type, "rise": _rise, "pop": _pop, "drop": _drop, "flip": _flip, "fade": _fade, "none": None}


def text(content, size=120, x=0, y=0, color="#ffffff", accent=None, fontname="inter-bold", preset="rise", start=0.0, stagger=0.03,
         tracking=-0.01, max_width=None, line_gap=1.12, align="center", exit_at=None, exit_d=0.35, name="text"):
    """A text layer of vector glyphs, centred on (x, y).

    `[[word]]` marks words drawn in the accent colour. preset: rise | pop | drop | flip | fade | none.
    `stagger` is the seconds between letters; `exit_at` fades the whole block out from that time.
    Returns a Layer (add it to a Doc).
    """
    fd = font(fontname)
    k = size / fd["upm"]
    cap = fd["capHeight"] * k
    # tokens: (char, is_accent)
    toks, acc, i = [], False, 0
    while i < len(content):
        if content.startswith("[[", i):
            acc, i = True, i + 2
        elif content.startswith("]]", i):
            acc, i = False, i + 2
        else:
            toks.append((content[i], acc))
            i += 1
    # words for wrapping
    words, cur = [], []
    for ch, a in toks:
        if ch == " ":
            words.append(cur)
            cur = []
        elif ch == "\n":
            words.append(cur)
            words.append(None)
            cur = []
        else:
            cur.append((ch, a))
    words.append(cur)
    adv = lambda ch: (fd["glyphs"].get(ch) or fd["glyphs"].get("?"))["adv"] * k + tracking * size
    space = adv(" ")
    lines, line, lw = [], [], 0.0
    for w in words:
        if w is None:
            lines.append(line)
            line, lw = [], 0.0
            continue
        ww = sum(adv(ch) for ch, _ in w)
        if line and max_width and lw + space + ww > max_width:
            lines.append(line)
            line, lw = [], 0.0
        if line:
            line.append((" ", False))
            lw += space
        line.extend(w)
        lw += ww
    lines.append(line)
    total_h = (len(lines) - 1) * size * line_gap
    glyph_total = sum(1 for ln in lines for ch, _ in ln if ch != " ")
    fn = PRESETS.get(preset)
    groups, gi = [], 0
    for li, ln in enumerate(lines):
        width = sum(adv(ch) for ch, _ in ln) - (tracking * size if ln else 0)
        px = {"center": -width / 2, "left": 0, "right": -width}[align]
        base = -total_h / 2 + li * size * line_gap + cap / 2
        for ch, a in ln:
            a_w = adv(ch)
            if ch == " ":
                px += a_w
                continue
            shapes, g_adv = _glyph_shapes(fd, ch, k, accent if (a and accent) else color)
            cx, cy = px + g_adv / 2, base - cap / 2
            m = fn(gi, glyph_total, start, stagger) if fn else dict(dy=None, opacity=100, scale=None, rot=None)
            pos = (cx, cy) if m.get("dy") is None else Anim([(kk[0], (cx, cy + kk[1] * size), *kk[2:]) for kk in m["dy"].keys])
            groups.append(group(shapes, pos=pos, anchor=(g_adv / 2, -cap / 2), scale=m.get("scale") or (100, 100), rot=m.get("rot") or 0,
                                opacity=m.get("opacity", 100), name=f"{ch}"))
            px += a_w
            gi += 1
    op = 100
    if exit_at is not None:
        op = Anim([(exit_at, 100, "inout"), (exit_at + exit_d, 0)])
    return Layer(groups, name=name, pos=(x, y), opacity=op)


def text_size(content, size, max_width=None, fontname="inter-bold", tracking=-0.01, line_gap=1.12):
    """(width, height) the text block takes, with `[[ ]]` markup ignored: for sizing a Doc around it."""
    plain = content.replace("[[", "").replace("]]", "")
    fd = font(fontname)
    lines, cur, lw = 1, 0.0, 0.0
    widest = 0.0
    space = measure(" ", size, fontname, tracking) + tracking * size
    for w in plain.replace("\n", " \n ").split(" "):
        if w == "\n":
            lines, lw = lines + 1, 0.0
            continue
        ww = measure(w, size, fontname, tracking) + (tracking * size if w else 0)
        if lw and max_width and lw + space + ww > max_width:
            lines, lw = lines + 1, 0.0
        lw += (space if lw else 0) + ww
        widest = max(widest, lw)
    return widest, size * fd["capHeight"] / fd["upm"] + (lines - 1) * size * line_gap


# --------------------------------------------------------------------------- motion helpers
def spring_scale(t, d=0.6, start=70, over=108):
    """Scale keys that arrive from `start` %, overshoot to `over` %, dip a hair under 100 and settle."""
    return [(t, (start, start), "out"), (t + d * 0.5, (over, over), "inout"), (t + d * 0.78, (98.4, 98.4), "inout"), (t + d, (100, 100))]


def idle_keys(base, t0, t1, ax=0.0, ay=6.0, period=4.0, phase=0.0, step=0.5):
    """Small endless-feeling drift as keyframes (Lottie has no expressions in Skottie): positions around `base`."""
    keys, t = [], t0
    w = 2 * math.pi / period
    while t <= t1 + 1e-6:
        keys.append((t, (base[0] + ax * math.sin(w * (t - t0) + phase), base[1] + ay * math.cos(w * 0.82 * (t - t0) + phase)), "inout"))
        t += step
    return keys


def mono_text(content, size, **kw):
    kw.setdefault("fontname", "jetbrains-mono")
    kw.setdefault("tracking", 0.0)
    return text(content, size=size, **kw)
