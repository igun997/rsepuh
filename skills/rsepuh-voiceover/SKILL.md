---
name: rsepuh-voiceover
description: Use when adding a voice-over (text to speech) to an rsepuh video, choosing between the free edge engine and Gemini TTS, writing voice lines, keeping speech in sync with subtitles and scene lengths, picking Indonesian or English voices, or fixing voice errors (HTTP 403, missing key, drift).
---

# Voice-over that stays in sync

The voice drives the timeline. Each scene has at most one `voice` line; `rsepuh voice` makes one audio clip per scene, **measures it**, and stores the clip length and every word's time in `assets/voice/NAME/timeline.json`. `rsepuh render` then:

- sets each scene's length to `max(dur, lead + clip length + tail)`;
- places the clip at the scene start plus `lead`;
- builds subtitle cues from the word times, so highlights land on the spoken word;
- ducks the music bed under every clip.

Because each scene is its own clip, sync between picture, sound and words is exact at scene level for both engines. Inside a scene, word accuracy depends on the engine.

## Engines

| | `edge` (free, default) | `gemini` |
|---|---|---|
| Cost | free | Gemini API key (usage billed by Google) |
| Needs | `uv` (https://docs.astral.sh/uv/) or a current `pip install -U edge-tts`; internet | `GEMINI_API_KEY` or `GEMINI_KEY` in env, `--env FILE` or `./.env` |
| Word times | real boundaries from the service | estimated from the trimmed speech: proportional to word length, pauses after punctuation |
| Typical word error | well under 0.1 s | about 0.1 to 0.3 s inside a long sentence; keep lines to one sentence |
| Voices | hundreds: `rsepuh voices --lang id` or `--lang en` | 30 prebuilt (`rsepuh voices --engine gemini`), default Kore |
| Direction | `rate` (`+10%`), `pitch` (`+2Hz`) | `style`, e.g. `"upbeat, fast"`, `lang` as BCP-47 (`id-ID`) |

Spec:

```json
"voice": { "engine": "edge", "lang": "id-ID", "voice": "id-ID-ArdiNeural", "rate": "+5%", "lead": 0.15, "tail": 0.35 }
"voice": { "engine": "gemini", "voice": "Kore", "lang": "en-US", "style": "warm, confident, medium pace" }
```

Defaults: edge picks `id-ID-ArdiNeural` for `id*` languages and `en-US-EmmaMultilingualNeural` otherwise; run `rsepuh voices --lang id` to see the others (for example `id-ID-GadisNeural`). The Gemini model is `gemini-3.8-flash-tts`; `model` can override it, and any video model name (Veo) is refused.

## Intonation (Gemini)

Gemini TTS follows natural-language direction, so intonation is written, not tuned. Set a base in `voice.style` and override per scene with `voiceStyle`:

```json
"voice": { "engine": "gemini", "voice": "Puck", "lang": "en-US", "style": "confident and punchy" },
{ "voice": "Edit like a veteran.", "voiceStyle": "confident, punchy, a little swagger" },
{ "voice": "EffectCraft. Blender. Gemini.", "voiceStyle": "excited and rhythmic, rising intonation" },
{ "voice": "Edit layaknya sepuh.", "voiceLang": "id-ID", "voiceStyle": "warm, proud, slowing down on the last word, falling intonation" }
```

- Describe pace, emotion and the contour of the sentence ("lift on the last words", "falling intonation", "pause before the last word").
- A hook is rising and punchy, a list is rhythmic with equal weight, a call to action is warm and falling.
- `voiceLang` (BCP-47) lets one line in another language (an Indonesian tagline inside an English video) be pronounced properly.
- Voices differ in character (Puck upbeat, Kore firm, Zephyr bright, Charon informative, Leda youthful, Fenrir excitable: listen before committing). `rsepuh voices --engine gemini` lists all 30.
- Punctuation is intonation too: full stops fall, commas hold, question marks rise.

Word timing for Gemini is anchored on the real pauses in each clip (silences of 80 ms or more are matched to your commas and full stops), then spread by word length inside each stretch, so a sentence with punctuation is accurate to about a word; a long unpunctuated run is the weak case.

## Writing voice lines

- One sentence, 5 to 12 words per scene. Short lines keep gemini estimates tight and make cuts land.
- Write numbers and acronyms the way they should sound: "three D", "A P I", "ten seconds". The subtitle shows the text you wrote, so keep it readable too, or put the readable form in `say` and silence the voice line with a separate scene.
- Punctuation is direction: a comma is a short pause, a full stop a long one. The word timing and the subtitle breaks both follow it.
- Indonesian text with an Indonesian voice; do not rely on an English voice for long Indonesian lines. A single borrowed phrase is fine.
- Brand names that a voice mispronounces: respell in the voice line ("R sepuh") and use `say` or the scene's own text for the written form.

## Workflow

1. Write `voice` lines in the spec. `rsepuh voice SPEC` (re-run after any change; unchanged lines are kept, changed ones are re-synthesised, `--force` redoes all).
2. `rsepuh render SPEC --draft`. If a scene's voice is out of date the build stops and names the command to run.
3. Check sync: read the cue list (`rsepuh subs SPEC`, then `exports/NAME/NAME.srt`), pick a time inside a word, `rsepuh frame SPEC --at T`, confirm the highlighted word.
4. Tune `rate`, `lead`, `tail` rather than hand-editing the timeline.

## Mixing

- Music: `"music": { "file": "assets/music/bed.mp3", "gainDb": -14, "duckDb": -9 }`. The bed ducks 0.3 s before each voice clip and returns 0.5 s after; it fades out in the last second.
- Formats EffectCraft reads: MP3, WAV, and the other common audio formats; the clips rsepuh makes are MP3 (edge) and 16-bit WAV (gemini).
- Loudness: the voice is not normalised. Measure the final MP4 (`ffmpeg -i X.mp4 -af volumedetect -f null -`, when ffmpeg is available) and report the mean volume; short-video platforms aim for about -14 LUFS. If the voice is too quiet, say so and use a louder voice or an `edge` `volume`-neutral rate change rather than guessing gains.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `edge-tts failed ... 403` | An old edge-tts release (Microsoft rotates its handshake). Run through `uv` (rsepuh does when `uv` exists) or `pip install -U edge-tts` |
| "needs `uv` ... or edge-tts" | Install uv, or use `engine: gemini` |
| "Gemini TTS needs GEMINI_API_KEY" | Set the key in env or `./.env` (git-ignored, never synced); never paste it into chat |
| Gemini HTTP 400 | Unknown voice or model name, or an unsupported `lang`; use a listed voice |
| "voice clip ... out of date" | The text or voice settings changed after `rsepuh voice`; run it again |
| Subtitle highlight early or late (gemini) | Estimated timings; shorten the line, or use edge |
| Scene longer than planned | Voice plus lead and tail exceeds `dur`; shorten the line or lower `tail` |
