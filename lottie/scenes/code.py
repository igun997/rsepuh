"""code: a code window whose tokens type in with syntax colours, a focus bar that glides between the
lines being discussed, and callout cards saying what they do.

  { "type": "code", "dur": 8, "file_name": "App.jsx",
    "code": ["useEffect(() => {", "  document.title = `Count: ${count}`;", "}, [count]);"],
    "startBeat": 0, "lineBeats": 0.75,
    "focus": [ { "line": 1, "atBeat": 4, "untilBeat": 8, "label": "syncs the tab title" } ] }
"""
import re
from rs import *

KW = {"const", "let", "var", "return", "function", "if", "else", "import", "from", "async", "await", "new", "true", "false", "null", "export", "default"}
TOK = re.compile(r"(\s+)|(//.*$)|(`[^`]*`|'[^']*'|\"[^\"]*\")|([A-Za-z_$][\w$]*)|(\d+)|(=>|.)")


def tokenize(line, ctx):
    out = []
    for m in TOK.finditer(line):
        if m.group(1):
            out.append((m.group(0), None))
        elif m.group(2):
            out.append((m.group(0), ctx.th("muted")))
        elif m.group(3):
            out.append((m.group(0), ctx.th("codeString", "#7ee787")))
        elif m.group(4):
            w = m.group(4)
            nxt = line[m.end():m.end() + 1]
            out.append((w, ctx.th("accent2") if w in KW else (ctx.th("accent") if nxt == "(" else ctx.th("fg"))))
        elif m.group(5):
            out.append((m.group(0), ctx.th("codeNumber", "#d2a8ff")))
        else:
            out.append((m.group(0), ctx.th("accent2") if m.group(0) == "=>" else ctx.th("muted")))
    return out


def build(doc, sc, ctx):
    code = sc["code"]
    n = len(code)
    S, W, H = ctx.S, ctx.W, ctx.H
    maxlen = max(len(l) for l in code) or 1
    fs = min(sc.get("size", 40), (W / S * 0.94 - 190) / (maxlen * 0.6)) * S
    cw = fs * 0.6
    line_h, win_w, left, head_h = fs * 1.72, W * 0.94, W * 0.03, ctx.px(118)
    win_h = head_h + n * line_h + ctx.px(56)
    card_h = ctx.px(214)
    block = win_h + ctx.px(70) + card_h
    top = H * 0.44 - block / 2 + ((sc["y"] - 0.44) * H if sc.get("y") else 0)
    cy = top + win_h / 2
    line_y = lambda i: top + head_h + (i + 0.5) * line_h
    card_y = top + win_h + ctx.px(70) + card_h / 2
    beat = ctx.beat or 0.5
    # window
    shapes = [panel_rect(ctx, win_w, win_h, ctx.th("panel"), ctx.px(36), ring=ctx.th("accent"), ring_w=3, ring_opacity=55)]
    for d, c in enumerate(["#ff5f57", "#febc2e", "#28c840"]):
        shapes.insert(0, group([ellipse(ctx.px(26)), fill(c)], pos=(-win_w / 2 + ctx.px(60 + 44 * d), -win_h / 2 + ctx.px(58)), name="dot"))
    tab = mono_text(sc.get("file_name", "App.jsx"), ctx.px(32), x=0, y=0, align="left", color=ctx.th("muted"), preset="none")
    shapes.insert(0, as_group(tab, (-win_w / 2 + ctx.px(230), -win_h / 2 + ctx.px(58))))
    wx = W / 2
    pos = Anim([(0.0, (wx, cy + ctx.px(50)), "out"), (0.55, (wx, cy), "out")] + idle_keys((wx, cy), 0.55, sc["dur"], 0, ctx.px(5), 6.0)[1:])
    doc.add(Layer(shapes, name="window", pos=pos, scale=Anim(spring_scale(0.0, 0.55, 92, 102.5)), opacity=Anim([(0.0, 0, "out"), (0.3, 100)])))
    # one bar that glides between the focus lines
    focus = sc.get("focus") or []
    if focus:
        fa, fz = focus[0]["atBeat"] * beat, focus[-1]["untilBeat"] * beat
        keys = [(fa - 0.05, (W / 2, line_y(focus[0]["line"])), "out")]
        for k in range(len(focus)):
            f = focus[k]
            if k + 1 < len(focus):
                keys.append((focus[k + 1]["atBeat"] * beat - 0.38, (W / 2, line_y(f["line"])), "out"))
                keys.append((focus[k + 1]["atBeat"] * beat, (W / 2, line_y(focus[k + 1]["line"])), "out"))
        keys.append((fz + 0.1, keys[-1][1]))
        doc.add(Layer([group([rect((win_w - ctx.px(36), line_h), (0, 0), ctx.px(12)), fill(ctx.th("accent"))], name="bar")], name="focus", pos=Anim(keys),
                      opacity=Anim([(fa - 0.05, 0, "out"), (fa + 0.2, 22, "linear"), (fz - 0.1, 22, "out"), (fz + 0.1, 0)]), start=fa - 0.05, end=fz + 0.12))
    # code
    start = sc.get("startBeat", 0) * beat
    lb = sc.get("lineBeats", 0.75) * beat
    x0 = left + ctx.px(104)
    for i, line in enumerate(code):
        ls = start + i * lb
        doc.add(_at(mono_text(str(i + 1), ctx.px(32), x=left + ctx.px(62), y=line_y(i), color=ctx.th("muted"), preset="none", name=f"ln-{i}"), ls))
        col, L = 0, max(len(line), 1)
        for tok, c in tokenize(line, ctx):
            if c is not None and tok.strip():
                ta = ls + col / L * lb * 0.85
                doc.add(_at(mono_text(tok, fs, x=x0 + col * cw, y=line_y(i), align="left", color=c, preset="none", name="tok"), ta))
            col += len(tok)
    # callouts
    for k, f in enumerate(focus):
        if not f.get("label"):
            continue
        ga, gb = f["atBeat"] * beat, f["untilBeat"] * beat
        card = [panel_rect(ctx, W * 0.86, card_h, ctx.th("panel"), ctx.px(36), ring=ctx.th("accent"), ring_w=5)]
        num = text(str(k + 1), size=ctx.px(70), x=0, y=0, color=ctx.th("accent"), preset="none")
        lab = text(f["label"], size=fit_size(ctx, [f["label"]], ctx.px(62), W * 0.86 - W * 0.14), x=0, y=0, color=ctx.th("fg"), preset="none", align="left")
        edge = -W * 0.86 / 2
        card = [as_group(lab, (edge + W * 0.13, 0)), as_group(num, (edge + W * 0.07, 0))] + card
        doc.add(Layer(card, name=f"callout-{k}", pos=Anim([(ga, (W / 2, card_y + ctx.px(50)), "out"), (ga + 0.5, (W / 2, card_y))]),
                      scale=Anim(spring_scale(ga, 0.5, 88, 104)),
                      opacity=Anim([(ga, 0, "out"), (ga + 0.2, 100, "linear"), (gb - 0.1, 100, "out"), (gb + 0.15, 0)]), start=ga, end=gb + 0.2))


def _at(layer, t):
    layer.start = t
    return layer
