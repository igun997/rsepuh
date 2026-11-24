"""Shared stage for rsepuh Blender scripts: transparent turntable-style renders that EffectCraft
composites as an image sequence. Import it from your own scripts:

    import sys, os; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import stage

Everything renders on the remote machine through rblender (CPU, no display needed).
"""
import math
import os

import bpy
import rb

PALETTE = {
    "gold": (1.0, 0.68, 0.22),
    "silver": (0.91, 0.92, 0.94),
    "copper": (0.95, 0.64, 0.54),
    "neon": (0.0, 0.9, 1.0),
    "rose": (1.0, 0.45, 0.6),
}
FONT_CANDIDATES = [
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
]


def parse(defaults):
    """Parse `--key value` pairs after `--` into the defaults dict (types follow the defaults)."""
    out = dict(defaults)
    a = rb.args()
    i = 0
    while i < len(a):
        if a[i].startswith("--") and i + 1 < len(a):
            key = a[i][2:].replace("-", "_")
            if key in out:
                cast = type(out[key])
                out[key] = cast(a[i + 1]) if cast is not bool else a[i + 1].lower() in ("1", "true", "yes")
                i += 2
                continue
        i += 1
    return out


def color(spec):
    if spec in PALETTE:
        return PALETTE[spec]
    s = spec.lstrip("#")
    if len(s) == 6:
        return tuple(int(s[k:k + 2], 16) / 255 for k in (0, 2, 4))
    raise SystemExit(f"stage: unknown colour {spec!r} (use {', '.join(PALETTE)} or #rrggbb)")


def metal(name, rgb, roughness=0.22, metallic=1.0):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    b = mat.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*rgb, 1.0)
    b.inputs["Metallic"].default_value = metallic
    b.inputs["Roughness"].default_value = roughness
    return mat


def assign(obj, mat):
    obj.data.materials.clear()
    obj.data.materials.append(mat)


def link(obj):
    bpy.context.scene.collection.objects.link(obj)
    return obj


def add_text(body, size=1.0, extrude=0.16, bevel=0.018, font=None, name="text"):
    cu = bpy.data.curves.new(name, "FONT")
    cu.body = body
    cu.size = size
    cu.extrude = extrude
    cu.bevel_depth = bevel
    cu.bevel_resolution = 4
    cu.align_x = "CENTER"
    cu.align_y = "CENTER"
    path = font or next((p for p in FONT_CANDIDATES if os.path.exists(p)), None)
    if path and os.path.exists(path):
        cu.font = bpy.data.fonts.load(path)
    obj = link(bpy.data.objects.new(name, cu))
    bpy.context.view_layer.update()
    return obj


def stand_up(obj):
    """Text and discs are built flat; stand them upright facing the camera (-Y)."""
    obj.rotation_euler = (math.radians(90), 0, 0)


def softbox(name, loc, size, strength):
    """A bright emissive panel outside the camera's view: it only shows up as a reflection in metal."""
    me = bpy.data.meshes.new(name)
    h = size / 2
    me.from_pydata([(-h, 0, -h), (h, 0, -h), (h, 0, h), (-h, 0, h)], [], [(3, 2, 1, 0)])
    ob = link(bpy.data.objects.new(name, me))
    ob.location = loc
    # the panel's normal is its local -Y... aim it at the origin
    ob.rotation_euler = (-ob.location).to_track_quat('Y', 'Z').to_euler()
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    nodes.remove(nodes["Principled BSDF"])
    em = nodes.new("ShaderNodeEmission")
    em.inputs["Strength"].default_value = strength
    mat.node_tree.links.new(em.outputs[0], nodes["Material Output"].inputs["Surface"])
    ob.data.materials.append(mat)
    ob.visible_camera = False
    return ob


def lights_and_world(strength=0.5):
    """Dark studio: metal gets its look from a few big soft panels, so it reads as metal, not as beige plastic."""
    scene = bpy.context.scene
    world = bpy.data.worlds.new("rsepuh-world")
    scene.world = world
    world.use_nodes = True
    bg = world.node_tree.nodes["Background"]
    bg.inputs[0].default_value = (0.04, 0.045, 0.06, 1.0)
    bg.inputs[1].default_value = strength
    softbox("panel-left", (-3.2, -3.0, 0.4), 3.0, 14.0)
    softbox("panel-right", (3.4, -2.6, -0.3), 2.2, 9.0)
    softbox("panel-top", (0.0, -2.8, 3.4), 3.4, 12.0)
    for name, loc, energy, size in [
        ("key", (-3.2, -4.2, 3.4), 700, 3.5),
        ("rim", (3.8, -2.0, 2.2), 500, 2.5),
    ]:
        data = bpy.data.lights.new(name, "AREA")
        data.energy = energy
        data.size = size
        o = link(bpy.data.objects.new(name, data))
        o.location = loc
        direction = -o.location
        o.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()


def camera_fit(width, height, margin=1.22):
    cam = bpy.data.cameras.new("cam")
    cam.type = "ORTHO"
    cam.ortho_scale = max(width, height) * margin
    o = link(bpy.data.objects.new("cam", cam))
    o.location = (0, -8, 0)
    o.rotation_euler = (math.radians(90), 0, 0)
    bpy.context.scene.camera = o
    return o


def swing(obj, frames, yaw=34.0, tilt=5.0):
    """Looping yaw swing (+-yaw degrees) with a small tilt: first frame == frame after the last."""
    for f in range(1, frames + 1):
        t = (f - 1) / frames
        obj.rotation_euler.z = math.radians(yaw) * math.sin(2 * math.pi * t)
        obj.rotation_euler.y = math.radians(tilt) * math.sin(2 * math.pi * t + 1.1)
        obj.keyframe_insert("rotation_euler", index=2, frame=f)
        obj.keyframe_insert("rotation_euler", index=1, frame=f)


def render(name, frames, size, engine="CYCLES", samples=32, fps=30):
    """Transparent RGBA PNG sequence `out/<name>/<name>_0001.png` ... plus report.json."""
    scene = bpy.context.scene
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.compression = 15
    path = rb.render_anim(f"{name}/{name}", frames=(1, frames), engine=engine, samples=samples, res=(size, size), fps=fps)
    rb.report(sequence=f"{name}/{name}_####.png", frames=frames, size=size)
    return path
