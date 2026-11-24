---
name: rsepuh
description: Use for ANY request to make, edit or render a short motion video (UGC style hooks, kinetic captions, stickers, terminal/stat scenes, end cards, 3D seals, PVs, reels, TikToks, shorts) with EffectCraft and Blender. Videos build and render on a remote sandbox through the `rsepuh` CLI (built on rcargo/rblender); never install or run EffectCraft or Blender on the user's computer. Also use when an rsepuh, rcargo or rblender command prints "SETUP REQUIRED".
---

# rsepuh: Edit layaknya sepuh

A project is a folder with `scenes/NAME.json` specs. `rsepuh render NAME` resolves the spec (timeline, voice, beats), sends it to the sandbox where a Python **motion writer** writes one **Lottie** file per scene (kinetic text, illustrations, cards, code windows), EffectCraft imports them and composes them with backgrounds, footage, camera moves, subtitles and audio, renders H.264 and pulls back `renders/NAME/NAME.mp4` and a contact sheet PNG. The motion lives in the project's `lottie/` folder; see the `rsepuh-lottie` skill to write or change it. Motion is handcrafted: there is **no video generation** (never Veo, never any video model). Gemini is optional and only for still images and the voice.

## First run

1. `rsepuh init` in an empty folder (Cargo stub, `scenes/`, skills, `.mcp.json`, Blender scripts, `rcargo init`).
2. `rsepuh doctor`. If no sandbox exists, ask the user which they want and recommend instabox (`npx instabox@latest`, then `rcargo instances add main --backend instabox --default`): it has full internet, so EffectCraft and Blender both install. Daytona (`rcargo instances add dt --backend daytona --set cpu=4 --set memory=8 --set disk=10 --default`, needs `daytona login` or `DAYTONA_API_KEY`, which only the user can provide) renders 2D videos fine, but some Daytona sandboxes block outbound hosts: in testing GitHub (EffectCraft) was reachable while download.blender.org was reset, so Blender could not install there. Any SSH host (`--backend ssh --set host=USER@HOST`) also works. If a Daytona sandbox stops answering after a restart (`Connection closed by remote host`), delete its cached token `~/.cache/rcargo/ssh/daytona-NAME.json` and retry.
3. EffectCraft installs itself on the sandbox on first use (under a minute, no extra libraries). Blender needs `rblender` (https://github.com/igun997/rblender) and installs on first 3D use.

Never ask the user to paste secrets into chat. Keys go in the environment or a `.env` file that is git-ignored and never synced to the sandbox.

## Commands

| Command | Use |
|---|---|
| `rsepuh styles` | motion styles, scene types, subtitle styles, voice engines |
| `rsepuh new NAME --style punch --aspect 9:16 --text "HOOK WORDS"` | starter spec |
| `rsepuh music SPEC` | handcrafted music bed + sound effects on the spec's beat grid (needs `bpm`) |
| `rsepuh voice SPEC` | speak each scene's `voice` line and time the words (see skill `rsepuh-voiceover`) |
| `rsepuh render SPEC --draft` | half-resolution iteration; drop `--draft` for the final |
| `rsepuh frame SPEC --at 1.4` | one preview PNG |
| `rsepuh subs SPEC` | write SRT/VTT sidecars (see `rsepuh-subtitles`) |
| `rsepuh blender medal --out seal` | 3D PNG sequence with alpha into `assets/3d/seal/` |
| `rsepuh gen image "prompt" --aspect 9:16 --out assets/gen/bg.png` | Gemini still (optional) |
| `rsepuh verify renders/NAME` / `rsepuh check FILE.mp4` | prove the render, test platform limits (see `rsepuh-publishing`) |
| `rsepuh ec -- ARGS` | raw `effectcraft-cli` on the sandbox |
| `rsepuh build SPEC [--plan]` | print the compositor script, or with `--plan` the motion writer's input (debugging) |

Choose a machine with `-i NAME` or `RCARGO_INSTANCE`. `rsepuh down` pauses the sandbox.

## The spec

```json
{
  "name": "launch", "aspect": "9:16", "fps": 30, "style": "punch",
  "voice": { "engine": "edge", "lang": "en-US" },
  "subtitles": { "style": "karaoke" },
  "music": { "file": "assets/music/bed.mp3", "gainDb": -14, "duckDb": -9 },
  "scenes": [
    { "type": "hook", "dur": 2.0, "words": ["STOP", "SCROLLING"], "voice": "Stop scrolling.", "subtitle": false },
    { "type": "chips", "dur": 2.6, "title": "Why", "items": ["Fast", "Honest", "Yours"], "voice": "Fast. Honest. Yours." },
    { "type": "cta", "dur": 2.4, "title": "launch", "caps": false, "button": "Try it", "url": "example.com" }
  ]
}
```

**Beat grid** (cut like an editor): add `"bpm": 120` and every scene length snaps up to a whole beat (`"snap": "bar"` for whole bars), pops, hook words, chips and buttons land on beats, the voice starts on the downbeat, and `"music": { "gen": "house" }` (or `lofi`) plus `rsepuh music SPEC` writes a bed whose kick sits exactly on that grid together with `NAME-sfx.wav` (impact on every cut, a whoosh into it, ticks and pops on hits). At 120 BPM a beat is 0.5 s, so 20 beats is 10 s. Order: `voice` (it fixes scene lengths) then `music` then `render`. Your own music: `"music": "assets/music/x.mp3"` (no grid guarantee; pick scene lengths from its tempo by hand).

Scene types: `hook`, `title`, `media`, `chips`, `terminal`, `stat`, `caption`, `cta`, `code`, `lottie` (a Lottie file you authored), `custom` (your own Python writer or a Lottie file); `rsepuh styles` lists each one's fields. Per scene: `at`, `transition` (`cut|flash|accent|wipe|dip`; on a beat grid every non-cut transition gets a whoosh and an impact on the cut), `caps:false` to keep casing, `voice` (spoken), `say` (subtitle without sound), `subtitle:false`. `dur` is a minimum when `voice` is present: the scene grows to fit the voice plus lead and tail.

Assets must live inside the project (the sandbox only sees synced files). Image sequences: `assets/3d/seal/seal_####.png` (or `%04d`). Sizes are written for a 1080 px short side and scale with the frame.

**Styles**: `punch` (default, black/yellow, hard pops, shake), `smooth`, `neon`, `clean`. Add `styles/NAME.json` with only the keys you change (it inherits `punch`), or override per spec with `"theme": {...}`. Theme keys also drive the Lottie scenes (`fg muted accent accent2 onAccent panel bg bg2 caps tracking pop tilt stagger motion aurora transition camera`). For a layout the built-ins do not have, write `lottie/scenes/foo.py` with `build(doc, sc, ctx)` and use `{ "type": "custom", "file": "lottie/scenes/foo.py", "dur": 4 }`; for a Lottie authored elsewhere use `{ "type": "lottie", "file": "assets/lottie/x.json", "dur": 4 }`. Details and verification in the `rsepuh-lottie` skill.

## Workflow

1. `rsepuh new` or edit a spec; `rsepuh build SPEC > /dev/null` catches spec mistakes in under a second with an actionable message.
2. `rsepuh render SPEC --draft`, then **look at** `renders/NAME/NAME.sheet.png` (one frame per scene). Fix text overflow, contrast, crowded or empty frames.
3. For one problem frame: `rsepuh frame SPEC --at T --scale 0.6`.
4. Final `rsepuh render SPEC`, `rsepuh verify renders/NAME`, `rsepuh check renders/NAME/NAME.mp4`.

Report only what you observed: the verify output and what the sheet shows. Never claim sync or quality you did not check.

## Rules

- Never generate video. If asked for Veo or any video model, explain that rsepuh is handcrafted motion and offer Gemini stills plus EffectCraft/Blender motion.
- Keep Blender work on the sandbox (`rsepuh blender`); never run Blender locally. The motion writer also runs on the sandbox; running it locally is only for a quick look at JSON.
- Do not delete or recreate sandboxes (`rcargo remote destroy`) without the user asking.
- Fonts bundled with EffectCraft: Inter (Regular, Medium, SemiBold, Bold), JetBrains Mono, Noto Serif. The default themes use `Inter-Bold`.
