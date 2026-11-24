---
name: rsepuh-subtitles
description: Use when adding, styling, timing or exporting subtitles and captions for an rsepuh video: burned-in styles (karaoke, pop, box, outline, typewriter, marker, lower, glow), cues from voice lines or SRT/VTT files, readability rules, and sidecar SRT/VTT export for upload.
---

# Subtitles

Subtitles come from one timeline shared with the picture and the voice, so they cannot drift. Three sources feed the same cue list:

| Source | How | Word times |
|---|---|---|
| Voice lines | `"voice": "..."` on a scene, then `rsepuh voice SPEC` | edge engine: real word boundaries; gemini engine: estimated from the speech |
| On-screen only | `"say": "..."` on a scene | spread evenly over the scene in proportion to word length, pauses after commas and full stops |
| A file | `"subtitles": { "file": "assets/subs/x.srt" }` (SRT or VTT, optional `offset` seconds) | each cue's words are spread over the cue |

A scene with `"subtitle": false` shows none (use it on `hook` and `cta` scenes that already carry big text).

## Styles

Set `"subtitles": { "style": "...", "y": 0.76, "size": 84, "caps": true, "maxWords": 4, "maxChars": 28 }`.

| Style | Look | Good for |
|---|---|---|
| `karaoke` (default) | phrase on screen, the spoken word pops in the accent colour | talking points, most UGC |
| `pop` | one huge word at a time in the middle | hype, short punchy voice-over, gaming |
| `box` | phrase on a dark rounded box, spoken word highlighted | busy or bright backgrounds, readability first |
| `outline` | white text, black outline, no highlight | classic captions, footage, quiet motion |
| `typewriter` | phrase types out in the mono font | tech, terminal scenes, deadpan |
| `marker` | a highlighter bar lands on the spoken word | tutorials, emphasis on key terms |
| `lower` | small text on a dark strip across the bottom | documentary, news, long sentences, landscape |
| `glow` | karaoke with a glowing spoken word | `neon` style, night scenes, music |

Pick the style from the video's mood (see `rsepuh-video-editing`), then change colours by theme (`accent`, `fg`, `panel`, `onAccent`) rather than per style. Use different styles in different videos, not within one.

## Readability rules

- At most 2 lines and about 28 characters per line on a phone: the defaults (`maxWords` 4, `maxChars` 28) do that. Raise them only for landscape.
- Reading speed under about 17 characters per second; if a cue is shorter than its words need, lengthen the scene or split the voice line.
- Breaks fall after a sentence end or a silence over half a second, never in the middle of a name.
- Keep captions out of the platform UI: bottom 20% and right edge. `y` is a fraction of the height (default 0.76 vertical, 0.82 landscape; `lower` sits at 0.84 or 0.9).
- Casing follows the theme (`caps`). Brand names with specific casing: set `"caps": false` in `subtitles`.
- Indonesian and English both work; the cue text is the script text exactly as written, including punctuation.

## Burn-in vs sidecar

Burned-in captions (this engine) always show and look the same everywhere. Platforms also take a sidecar file for accessibility and search: `rsepuh render` does not write sidecar files and the MP4 has no subtitle track: what the viewer sees is the burned-in text. `rsepuh subs SPEC --format srt` writes `exports/NAME/NAME.srt` (and `.vtt`) for platforms that take a caption file (YouTube, LinkedIn, Facebook). They are kept out of the MP4's folder on purpose: video players auto-load a same-named subtitle file (VLC also scans `./subtitles` and `./Subs`), and the captions would appear twice. TikTok and Instagram use their own auto-captions, so burn-in is the reliable option there.

## Checking sync

1. `rsepuh render SPEC --draft`, open the contact sheet.
2. Pick a mid-word time from the cue list (`rsepuh subs SPEC` prints the SRT into `exports/NAME/`), then `rsepuh frame SPEC --at T`: the highlighted word must be the one the voice says at T.
3. If a gemini-engine voice drifts mid-sentence, shorten the voice line (one sentence per scene is the most accurate), or switch to `edge` for real word times.
4. `subtitles.enabled: false` keeps the SRT/VTT export but hides burn-in.

## Custom looks

Colours, fonts and stroke come from the theme. Subtitles are composited natively by EffectCraft (`phrase()` / `subtitleCue()` in `styles/compositor.jsx` of the rsepuh repo): cue words arrive as `{w, s, e}` with absolute seconds. A subtitle look the styles do not cover can be a Lottie overlay scene (`rsepuh-lottie` skill) driven by the cue times in the plan (`rsepuh build SPEC --plan` lists `cues`).
