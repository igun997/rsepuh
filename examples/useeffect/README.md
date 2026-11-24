# Example: "What useEffect actually does" (about 1:37, 9:16)

A long, story-driven explainer made entirely with rsepuh: Gemini voice (director's notes per scene), a
handcrafted 90 BPM lofi bed and sound effects on the beat grid, illustrated diagram and timeline written
as Lottie, kinetic type, typed code windows, subtitles that highlight the spoken word, and on-screen
moments pinned to the words that explain them (`onWord` / `onWords` / `atWord`).

```sh
rsepuh init                                   # in an empty folder; then pick a sandbox (instabox recommended)
cp -r EXAMPLE/lottie/scenes/* lottie/scenes/  # diagram.py, pipeline.py
cp EXAMPLE/styles/devnight.json styles/
cp EXAMPLE/scenes/useeffect.json scenes/
rsepuh voice useeffect --env ~/path/to/.env   # Gemini voice (needs GEMINI_API_KEY); or set engine "edge" for the free voice
rsepuh music useeffect                        # beat-grid music bed + sfx
rsepuh render useeffect --draft               # half-resolution check, then without --draft
```

- `lottie/scenes/diagram.py`: an atom and a globe that draw themselves on, tags that pop in, two-way arrows with travelling dots and a pulsing bridge.
- `lottie/scenes/pipeline.py`: a vertical timeline whose rail draws itself and whose current node pulses.
- `styles/devnight.json`: React-cyan and coral on near-black, sentence case, aurora light, push transitions.
