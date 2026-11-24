---
name: rsepuh-video-editing
description: Use when planning, scripting or critiquing a short vertical video (TikTok, Reels, Shorts, PV, ad, explainer) before or while building it with rsepuh. Covers hooks, pacing, scene structure, typography, motion feel, safe zones, music and 3D usage, and a review checklist. Pair with the rsepuh skill for commands.
---

# Editing like a sepuh

"Sepuh" is the one who has cut a thousand of these. The craft is mostly restraint: one idea per scene, one thing moving at a time, the point arrives early.

## Structure

| Format | Scenes (seconds) |
|---|---|
| 10 s PV / teaser | hook 2 / proof 3 / what-you-get 3 / call to action 2 |
| 20 to 30 s explainer | hook 2 / problem 4 / 3 steps (chips or terminal) 3 each / proof (stat) 3 / call to action 3 |
| Listicle | hook with the count / one `title` or `chips` scene per item, 2 to 3 s each / call to action |
| Demo | hook with the result / `terminal` or `media` of the real thing / `stat` / call to action |

Total length: shortest that carries the idea. Under 15 s loops well; under 3 s is rejected by Reels.

## Hook (first 1.5 s)

- Lead with the result, the question or the tension, never a logo or a greeting.
- `hook` scene: 2 to 4 words, one word at a time, `dur` 1.6 to 2.4. Alternate white and accent. Put the explanation in `sub`.
- Say the hook out loud in the voice line too; sound and text landing together is what stops a scroll.

## Pacing

- A new visual event at least every 1.5 to 3 s: a pop, a cut, a flash, a new word. Longer than 3 s without one reads as dead air.
- Do not move everything at once. Entrance, then settle; the eye follows one motion.
- Speech rate about 2.5 to 3.5 words per second. A voice line of N words needs about N/3 s plus 0.35 s of lead and tail; `dur` grows automatically, so set `dur` to the minimum you want.
- Cut on the beat of the voice: word boundaries are the edit points. `rsepuh voice` gives real word times for the free engine.

## Cutting on the beat

Real editors cut on the beat. Set `"bpm"` in the spec and rsepuh does the arithmetic:

- Scene lengths snap up to whole beats (`"snap": "bar"` for bars), so every cut is on the grid. At 120 BPM: 2 s = 4 beats, 3 s = 6 beats, 10 s = 20 beats = 5 bars. Choose `dur` in beats x (60 / bpm) and let the voice stretch a scene when it needs to.
- Hook words land on half beats, chips on one beat each, the call-to-action button on beat 3. All of it follows the same grid as the music.
- Plan the arrangement with the structure: hook on bar 1 (kick and hats), bass enters bar 2, claps and chords from bar 3, the end card on the last bar, a crash on the final downbeat.
- Voice starts on the downbeat of its scene; the next beat is the breath. If a line does not fit, shorten the line before widening the scene.
- Pick the tempo from the mood: 100 to 110 for confident and calm, 120 to 128 for ads and hype, 90 for lo-fi (`"gen": "lofi"`). A scene shorter than 4 beats rarely reads.

## Transitions that feel edited

A transition is a cover that peaks exactly on the cut, with sound. Use it to mark a change of idea, not on every cut.

| `transition` | Look | Use for |
|---|---|---|
| `cut` | nothing, no sound | continuing the same idea |
| `flash` | white flash on the downbeat | hits, reveals, hook to proof |
| `accent` | accent-colour flash | the call to action, brand moments |
| `wipe` | accent bar sweeps across and the cut happens under it | topic change, "now the how" |
| `dip` | quick dip to black | slower, calmer videos |

On a beat grid each non-cut transition automatically gets a whoosh that builds into an impact on the cut (`rsepuh music`).

## Typography and layout

- Short lines. One font weight (Bold) for headlines, Regular for supporting text; the mono font only for code or typed text.
- Contrast: white text carries a black stroke on busy images (punch style); on flat backgrounds pick `clean` or `smooth` and no stroke. `media` scenes dim the picture by 28% by default (`dim`).
- Keep the centre 80% of the width. Text is auto-fitted (`fit`) but a long phrase still gets small; split it across scenes instead.
- Vertical safe zones: platform UI covers about the bottom 20% and the right edge. Subtitles default to 76% of the height; important scene content sits between 15% and 70%.
- Caps for punchy styles (`punch`, `neon`), sentence case for calm ones (`smooth`, `clean`). Brand names keep their casing: `"caps": false` on that scene.

## Motion feel

| Want | Use |
|---|---|
| Aggressive, ad, hype | `punch`: pop 1.18, shake, `flash` transitions |
| Calm, lifestyle, explainer | `smooth`: slide-up, no shake, `cut` |
| Night, music, gaming | `neon`: accent flashes, tilted chips |
| SaaS, product demo | `clean`: light, quick pops, no shake |

Overshoot (pop above 100% then settle) reads as energy; slide-up reads as calm. Do not mix both in one video. Flash transitions only between ideas, not every scene.

## Media, 3D and stills

- A generated or filmed background goes behind everything with `dim`; the subject (3D seal, product cut-out) is an alpha PNG sequence as `overlay`.
- `rsepuh blender text3d` for extruded metal words, `rsepuh blender medal` for a seal. Keep sequences 48 to 60 frames at 30 fps; they loop (the swing is seamless). 560 to 720 px square is plenty for an overlay at 80% width.
- Gemini stills: ask for empty space where the text goes ("empty centre", "no text, no people"). Models paint text badly, so never ask for text inside an image; add it in the scene.
- AI-made stills should be disclosed where the platform asks (see the `rsepuh-publishing` skill).

## Audio

- On a beat grid, `music.gen` writes a bed whose kick is exactly on the beats and `NAME-sfx.wav` with cut impacts, whooshes and pops; both come from `rsepuh music SPEC` and need the voice timeline first.
- Voice first, music under it. `music.gainDb` -14 and `duckDb` -9 keep a bed audible without hiding words; the bed ducks around every voice window and fades out at the end.
- Use music you have the rights to. Platform libraries are the safe route for ads.
- No voice? Use `say` for on-screen words and a music bed at about -8 dB instead.

## Review checklist

- Is the point clear in the first 2 s with the sound off?
- Any scene that holds more than one idea, or lasts over 3.5 s without a visual event?
- Any text under 40 px at 1080 wide, outside the safe zone, or low contrast in the contact sheet?
- Do subtitles and the spoken word agree at a mid-scene frame (`rsepuh frame SPEC --at T`)?
- Does the end card name one action and show where (`button`, `url`)?
- `rsepuh check` passes for the target platforms.
