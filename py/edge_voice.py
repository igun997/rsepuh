"""Free speech with word timings through edge-tts (Microsoft Edge read-aloud service).

    edge_voice.py OUT.mp3 OUT.json VOICE RATE PITCH < text

Writes the MP3 and a JSON file {"words": [{"text", "offset", "duration"}]} (seconds). rsepuh runs
this with `uv run --with edge-tts` so it always uses the current edge-tts, which Microsoft's service
needs (old releases get HTTP 403).
"""
import asyncio
import json
import sys

import edge_tts


async def main():
    out, meta, voice, rate, pitch = sys.argv[1:6]
    text = sys.stdin.read().strip()
    comm = edge_tts.Communicate(text, voice, rate=rate, pitch=pitch, boundary="WordBoundary")
    words = []
    with open(out, "wb") as f:
        async for chunk in comm.stream():
            if chunk["type"] == "audio":
                f.write(chunk["data"])
            elif chunk["type"] == "WordBoundary":
                words.append({"text": chunk["text"], "offset": chunk["offset"] / 1e7, "duration": chunk["duration"] / 1e7})
    with open(meta, "w") as f:
        json.dump({"words": words}, f)


asyncio.run(main())
