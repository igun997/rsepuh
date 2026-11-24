---
name: rsepuh-lottie
description: Use when writing or changing the MOTION of an rsepuh video: kinetic text, illustrations, cards, code windows, diagrams. Motion is written as Lottie by a Python motion writer that runs on the sandbox (lottie/writer.py, lottie/kit.py, lottie/scenes/*.py), or authored with the text-to-lottie skill and used as a `lottie` scene. Also for verifying a scene in the Skottie player and for fixing stiff or badly proportioned motion.
---

# Motion as Lottie

Every scene's text, illustration and UI motion is a **Lottie** file. The pipeline on the sandbox:

```
spec + style --(rsepuh)--> plan.json --python3 lottie/writer.py--> scene-NN.json (one Lottie per scene)
                                                                    |
              EffectCraft imports each as a composition  <-----------+
              + backgrounds, footage, camera, subtitles, audio  -->  MP4
```

Nothing here runs on the user's computer except `rsepuh` itself: `rsepuh render` sends the plan to the sandbox, which runs the writer (Python standard library only), imports the Lottie into EffectCraft and renders. The writer and its fonts live in the project's `lottie/` folder, so they can be read and changed; `rsepuh` only copies files that are missing and never overwrites yours.

## Three ways to get motion

| Way | When | How |
|---|---|---|
| Built-in scene types | most videos | `hook title chips terminal stat caption cta media code` in the spec; style in `styles/*.json` |
| Your own Python writer | a layout or illustration the built-ins do not have | `{ "type": "custom", "file": "lottie/scenes/diagram.py", "dur": 9, ... }` with `build(doc, sc, ctx)` |
| A Lottie file authored elsewhere | an animation from the text-to-lottie skill, a designer or a library | `{ "type": "lottie", "file": "assets/lottie/x.json", "dur": 4 }` (scaled to fit, `"fit": "cover"` to fill) |

## Writing a scene (`lottie/scenes/NAME.py`)

```python
from rs import *                       # kit + helpers

def build(doc, sc, ctx):               # doc: transparent kit.Doc, video-sized, scene-long
    # ctx.W ctx.H ctx.S ctx.px(n) ctx.th("accent") ctx.beat ctx.cy (content centre) ctx.caps(sc, s)
    doc.add(text("Edit like a [[veteran]]", size=ctx.px(150), x=ctx.W/2, y=ctx.cy,
                 color=ctx.th("fg"), accent=ctx.th("accent"), preset="drop", start=0.1, stagger=0.03,
                 max_width=ctx.W*0.86, tracking=tracking(ctx)))
    doc.add(underline(ctx, ctx.W/2, ctx.cy + ctx.px(120), ctx.px(300), ctx.th("accent2"), 0.6))
```

Times inside a scene are seconds from the scene start (sc dict fields in beats are multiplied by `ctx.beat`). Sizes are written for a 1080 px short side: always wrap in `ctx.px(...)`.

### Kit reference (`lottie/kit.py`)

- Text is **vector glyphs** (Inter Bold, JetBrains Mono): no font is needed anywhere and every letter moves on its own. `text(content, size, x, y, color, accent, preset, start, stagger, max_width, align, tracking, exit_at)`; `[[word]]` uses the accent colour; presets `rise drop pop flip fade type none`. `mono_text(...)` for code.
- Shapes: `ellipse rect path bezier_path star`, styles `fill stroke trim` (trim = draw-on), `group(items, pos, anchor, scale, rot, opacity)`; layers `Layer(shapes, name, pos, scale, rot, opacity, start, end)`; `Doc(name, w, h, fps, seconds)`.
- Animation: `Anim([(t, value, ease), ...])` for any property. Eases: `out` (fast start, long landing), `out_soft`, `in`, `inout`, `spring` (overshoots once), `spring_hard`, `linear`, `hold`. Helpers: `spring_scale(t, d, start, over)`, `idle_keys(base, t0, t1, ax, ay, period)` (endless drift as keys), `rise_layer(layer, at)`.
- `rs.py` helpers: `chip` (pill that slides in with a spring), `underline`, `fit_size`, `panel_rect`, `as_group`.
- Skottie (the player) has no expressions: loops and idle motion are sampled keyframes, which also import cleanly into EffectCraft.

## Motion that feels fluid, not stiff

- Nothing appears at full size in place. Arrive from somewhere: rise 40 to 90 px, slide 150 to 200 px, scale from 70 to 80 percent, with a small tilt that settles.
- Use `out` or `spring` for entrances, `inout` for moves between two resting places, never `linear` except for steady spin.
- Per-letter stagger 20 to 40 ms; per-item stagger on the beat (`itemT`, word times from the voice).
- After it lands, keep a little life: `idle_keys` drift of 4 to 8 px, rotating or breathing shapes. Draw lines on with `trim` instead of fading them.
- Overshoot once, not twice: `spring_scale(t, 0.5, 80, 108)` is plenty.
- Entrances 0.4 to 0.7 s; whole-scene camera push, exit and transitions are done when compositing, so scenes do not need their own.

## Proportion

- One content zone: centre at `ctx.cy` (0.44 of the height), content between 0.12 and 0.78 of the height; subtitles own the area below 0.8.
- Type scale (1080 base): display 250 to 320, title 160 to 190, heading 90, body 50 to 60, small 40. Use `fit_size` so a long line shrinks instead of overflowing; give lines of one block the same size.
- Margins 6 to 7 percent. Do not stack text closer than 0.9 x its cap height to a neighbouring shape (underline gap 24 to 34 px, heading to tags 80+ px).
- Mono text advances 0.6 em per character: size a code window from its longest line.

## Verify before compositing (the text-to-lottie workflow)

Look at the writer's output in the official Skottie player, frame by frame:

```bash
npx degit diffusionstudio/lottie player && cd player && npm install && npm run dev     # http://localhost:3030
rsepuh build SPEC --plan --out plan.json
python3 lottie/writer.py plan.json /tmp/lottie --player player/public                  # or on the sandbox
# open /<spec name>/scene-<N>?frame=<F> for the first, middle and last frame of each scene
```

`--player` writes `player/public/projects/<name>/scene-N/lottie.json` with the `bgColor` / `bgOpacity` slots the player's panel expects; the files EffectCraft imports are flat (transparent, no slots). Check: first frame, a settled middle frame, the last frame; look for clipped text, overlaps, stray artifacts, motion that stops dead. Then `rsepuh render SPEC --draft` and look at the contact sheet; EffectCraft import warnings are in `renders/NAME/import.log`.

## Using a Lottie authored with text-to-lottie

Follow that skill (player project, `lottie.json`, slots, controls.json) and then, for rsepuh:

- Canvas the size of the video (`w`,`h`) or let `fit` scale it; 30 fps; transparent background (keep the background layer opacity at 0 or leave it out of the copy you use).
- Text as shapes (vector glyphs) unless the font is certain to exist; no expressions; shape layers, masks, mattes, trim paths and eased keyframes import fine.
- Copy it to `assets/lottie/NAME.json`, give the scene a `dur` that matches or loop it in the Lottie itself.

## Troubleshooting

| Symptom | Fix |
|---|---|
| "python3 is required on the sandbox" | install python3 there (`apt-get install python3`) |
| "the motion writer produced no Lottie for scene N" | the writer failed or skipped it: read the output above that line |
| A custom writer: "must define build(doc, sc, ctx)" | add the function; the module is imported from `lottie/scenes/` |
| Text missing | the character is not in the bundled font (ASCII plus common punctuation and arrows); use another character or add the glyph with `lottie/tools/extract_glyphs.py` |
| Layers in the wrong order | later `doc.add()` layers are on top |
| Motion looks stiff | see "Motion that feels fluid"; check every `Anim` has an ease other than `linear` |
