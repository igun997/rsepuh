#!/usr/bin/env python3
"""The motion writer. Turns the resolved plan of a spec into one Lottie file per scene.

    python3 lottie/writer.py PLAN.json OUT_DIR [--player PLAYER_PUBLIC_DIR]

rsepuh runs this on the sandbox, then EffectCraft imports the Lottie files and composites them with
the backgrounds, footage, camera moves, subtitles and audio. Every built-in scene type is a module in
lottie/scenes/ with `build(doc, sc, ctx)`; a scene of type "custom" runs your own module
(`"file": "lottie/scenes/mine.py"`), and type "lottie" (or a custom `.json`) uses a Lottie file you
authored elsewhere, for example with the text-to-lottie skill. Only the Python standard library is used.

With --player, each generated scene is also written for the official Skottie player project
(`npx degit diffusionstudio/lottie`, see the text-to-lottie skill) as
PLAYER_PUBLIC_DIR/projects/<spec name>/scene-<N>/lottie.json, with the background slots the player expects,
so you can look at it frame by frame (`/<spec name>/scene-<N>?frame=60`) before compositing.
"""
import importlib.util
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from rs import Ctx, Doc  # noqa: E402

BUILTIN = {"hook", "title", "chips", "terminal", "stat", "caption", "cta", "media", "code"}


def load_module(path):
    spec = importlib.util.spec_from_file_location("rsepuh_scene_" + os.path.basename(path).replace(".", "_"), path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    if not hasattr(mod, "build"):
        raise SystemExit(f"{path}: a scene writer must define build(doc, sc, ctx)")
    return mod


def main(plan_path, out_dir, player=None):
    plan = json.load(open(plan_path))
    ctx = Ctx(plan)
    root = os.getcwd()
    os.makedirs(out_dir, exist_ok=True)
    manifest = []
    for idx, sc in enumerate(plan["spec"]["scenes"]):
        ty = sc["type"]
        name = f"rs-{ctx.name}-s{idx:02d}"
        out = os.path.join(out_dir, f"scene-{idx:02d}.json")
        passthrough = sc.get("file") if (ty == "lottie" or (ty == "custom" and str(sc.get("file", "")).endswith(".json"))) else None
        if passthrough:
            doc = json.load(open(os.path.join(root, passthrough)))
            doc["nm"] = name
            json.dump(doc, open(out, "w"), separators=(",", ":"))
            manifest.append({"index": idx, "nm": name, "file": out, "w": doc["w"], "h": doc["h"], "fr": doc["fr"], "op": doc["op"]})
            continue
        if ty in BUILTIN:
            mod = load_module(os.path.join(HERE, "scenes", ty + ".py"))
        elif ty == "custom":
            mod = load_module(os.path.join(root, sc["file"]))
        else:
            raise SystemExit(f"scene {idx + 1}: unknown type {ty!r}")
        doc = Doc(name, ctx.W, ctx.H, fps=ctx.fps, seconds=sc["dur"])
        mod.build(doc, sc, ctx)
        doc.save(out)
        if player:
            doc.save_player(os.path.join(player, "projects", ctx.name, f"scene-{idx + 1}", "lottie.json"))
        manifest.append({"index": idx, "nm": name, "file": out, "w": ctx.W, "h": ctx.H, "fr": ctx.fps, "op": round(sc["dur"] * ctx.fps)})
    json.dump(manifest, open(os.path.join(out_dir, "manifest.json"), "w"), indent=1)
    print(f"rsepuh writer: {len(manifest)} scenes -> {out_dir}")


if __name__ == "__main__":
    args = sys.argv[1:]
    player = None
    if "--player" in args:
        k = args.index("--player")
        player = args[k + 1] if k + 1 < len(args) else None
        del args[k:k + 2]
    if len(args) != 2:
        raise SystemExit(__doc__)
    main(args[0], args[1], player)
