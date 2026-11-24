"""A struck metal medal with a star and a beaded rim, swinging in a seamless loop, rendered as a
transparent PNG sequence. A "sepuh" seal: the mark of someone who has done it a thousand times.

    rsepuh blender medal --out seal
    rsepuh blender medal --label SEPUH --color gold --frames 60

Options: --label --color (gold|silver|copper|neon|rose|#rrggbb) --frames --size --samples --engine --beads
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bmesh
import bpy
import rb
import stage

o = stage.parse({"label": "SEPUH", "color": "gold", "frames": 60, "size": 640, "samples": 32,
                 "engine": "CYCLES", "beads": 28, "name": "medal", "yaw": 34.0})
rb.clean()
rgb = stage.color(o["color"])
bright = stage.metal("bright", rgb, roughness=0.18)
deep = stage.metal("deep", tuple(c * 0.62 for c in rgb), roughness=0.34)


def bevelled(obj, width=0.025):
    mod = obj.modifiers.new("bevel", "BEVEL")
    mod.width = width
    mod.segments = 4
    return obj


def add_mesh(name, verts, faces, mat):
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, [], faces)
    me.update()
    ob = stage.link(bpy.data.objects.new(name, me))
    stage.assign(ob, mat)
    for p in me.polygons:
        p.use_smooth = False
    return ob


# the medal is built flat in XY (facing +Z), parented to a pivot, then stood up
pivot = stage.link(bpy.data.objects.new("pivot", None))

bpy.ops.mesh.primitive_cylinder_add(radius=1.0, depth=0.16, vertices=96)
disc = bevelled(bpy.context.active_object, 0.03)
disc.name = "disc"
stage.assign(disc, bright)

bpy.ops.mesh.primitive_cylinder_add(radius=0.82, depth=0.2, vertices=96)
inset = bpy.context.active_object
inset.name = "inset"
stage.assign(inset, deep)

bpy.ops.mesh.primitive_torus_add(major_radius=0.9, minor_radius=0.045, major_segments=96, minor_segments=20,
                                 location=(0, 0, 0.085))
ring = bpy.context.active_object
ring.name = "ring"
stage.assign(ring, bright)

# five point star, solid, sitting on the inset
pts, r_out, r_in, h = [], 0.5, 0.21, 0.13
for k in range(10):
    ang = math.pi / 2 + k * math.pi / 5
    r = r_out if k % 2 == 0 else r_in
    pts.append((r * math.cos(ang), r * math.sin(ang) + 0.17))
base_z = 0.1
verts = [(0, 0.17, base_z), (0, 0.17, base_z + h)]
verts += [(x, y, base_z) for x, y in pts] + [(x, y, base_z + h) for x, y in pts]
faces = []
for k in range(10):
    n = (k + 1) % 10
    faces.append((0, 2 + n, 2 + k))                    # bottom fan
    faces.append((1, 12 + k, 12 + n))                  # top fan
    faces.append((2 + k, 2 + n, 12 + n, 12 + k))       # side
star = add_mesh("star", verts, faces, bright)
bevelled(star, 0.012)

# beads around the rim
for k in range(o["beads"]):
    a = 2 * math.pi * k / o["beads"]
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.034, segments=14, ring_count=8,
                                         location=(0.955 * math.cos(a), 0.955 * math.sin(a), 0.0))
    b = bpy.context.active_object
    stage.assign(b, bright)
    b.parent = pivot

# label under the star
lab = stage.add_text(o["label"], size=0.2, extrude=0.03, bevel=0.004, name="label")
lab.location = (0, -0.5, 0.1)
stage.assign(lab, bright)

for ob in (disc, inset, ring, star, lab):
    ob.parent = pivot
pivot.rotation_euler = (math.radians(90), 0, 0)
bpy.context.view_layer.update()

stage.lights_and_world()
stage.camera_fit(2.0, 2.0, margin=1.12)
stage.swing(pivot, o["frames"], yaw=o["yaw"])
stage.render(o["name"], o["frames"], o["size"], engine=o["engine"], samples=o["samples"])
