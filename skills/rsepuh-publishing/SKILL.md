---
name: rsepuh-publishing
description: Use when exporting, checking and publishing a finished rsepuh video to TikTok, Instagram Reels, YouTube Shorts or LinkedIn: export settings, platform limit checks, captions, hashtags, titles, covers, AI-asset disclosure, scheduling, and what must be confirmed with the user before anything goes public.
---

# Publishing

## Export

`rsepuh render SPEC` (without `--draft`) already writes the file the three big short-video platforms accept: **1080x1920, H.264 MP4, AAC audio, 30 fps**, 8 Mbps by default (`--bitrate KBPS`, or `"bitrate"` in the spec). Upload the original export, not a copy downloaded from another app (it carries that app's watermark).

```
rsepuh verify renders/NAME          # MP4 parsed, frames decoded, subtitles valid
rsepuh check renders/NAME/NAME.mp4  # length, size, resolution, fps, codec, audio per platform
```

`rsepuh render` also prints each platform's fit at the end.

## Limits (checked 2026-11-06 against the platforms' API documentation via post0.ai)

| | TikTok | Instagram Reels | YouTube Shorts |
|---|---|---|---|
| Length | up to 3 min for every creator through the API (some accounts 5 or 10) | 3 s to 15 min | up to 3 min |
| File size | 4 GB | 1 GB | 256 GB |
| Frame | 9:16 recommended, each side 360 to 4096 px | 9:16, up to 1920 px wide | square or vertical only |
| fps | 23 to 60 | 23 to 60 | keep source |
| Caption | 2,200 chars incl. hashtags | 2,200 chars | description 5,000 bytes, title 100 chars |
| Cover | frame from the video | frame or image | frame from the video |

These change. `rsepuh check` encodes this table; before a launch confirm in the platform's own docs or app, and tell the user the date above.

## Caption, title, hashtags

- Line 1 is the hook again (it is what shows before "more"). Then one line of value, then the call to action.
- 3 to 5 specific hashtags beat 20 generic ones. Put them at the end.
- YouTube needs a title of 100 characters or fewer and a description; TikTok and Instagram use the caption as the title.
- Write the caption in the audience's language. Indonesian audience: Indonesian caption, English only where the product name is English.
- The subtitles are burned in. Only where a platform has its own caption upload (YouTube, LinkedIn, Facebook) and the user wants selectable captions, run `rsepuh subs SPEC` and upload `exports/NAME/NAME.srt`. Then tell the user the burned-in text and the platform captions will both show unless they turn one off. Never copy the SRT/VTT next to the MP4: players auto-load it and show every caption twice.
- Cover: pick the hook frame (`rsepuh frame SPEC --at 0.6 --scale 1`), or the end-card; keep text inside the centre so grid crops do not cut it.

## Disclosure and rights

- Gemini stills and synthetic voices are AI-generated content. TikTok, Instagram and YouTube each have an "AI-generated" label setting; turn it on when the video contains realistic AI imagery or voice, and say in the caption if the platform needs it. `assets/gen/ledger.jsonl` records every generated still (model, prompt, time).
- No video model is used by rsepuh; do not describe the motion as AI-generated video. It is handcrafted motion in EffectCraft and Blender.
- Music must be licensed for the platform. TikTok business accounts are restricted to the commercial sound library.
- Branded content (paid partnership) must be disclosed with the platform's own toggle.

## Confirm before anything public

Publishing, scheduling, replying and deleting affect real audiences. Before any of it:

1. Show the user the exact MP4 path, the caption, hashtags, title, cover choice, accounts and the time.
2. Wait for a clear yes. Do not infer approval from earlier steps.
3. After posting, report only what the tool returned (link, status), not what you expect.

## If a publishing tool is connected

Check which MCP tools exist before assuming. With a social planner such as MauKirim's content tools (`content_list_accounts`, `content_add_image` for covers by public https URL, `content_save_post` as draft or with `scheduledAt`, `content_publish_post` and `content_delete_post` with `confirm: true`, `content_report` for numbers), prefer saving a **draft** or a **scheduled** post and let the user approve the publish. Those tools take public URLs for media: an MP4 on the user's disk must be hosted first (ask where; never upload it to an unknown host). Without a connected tool, give the user the file, the SRT, and a ready caption block to paste into the app.

## After posting

Read results with the platform's analytics or `content_report`; judge by hook retention (3 s views) before likes. Change one variable per next video: hook wording, first scene, subtitle style, or length.
