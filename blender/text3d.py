"""Extruded metal 3D text, swinging in a seamless loop, rendered as a transparent PNG sequence.

    rsepuh blender text3d --text SEPUH --out seal
    rsepuh blender text3d --text "NEW DROP" --color silver --frames 48 --size 720

Options: --text --color (gold|silver|copper|neon|rose|#rrggbb) --frames --size --samples --engine --font
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
import rb
import stage

o = stage.parse({"text": "SEPUH", "color": "gold", "frames": 60, "size": 640, "samples": 32,
                 "engine": "CYCLES", "font": "", "name": "text3d", "yaw": 34.0})
rb.clean()
text = stage.add_text(o["text"], font=o["font"] or None)
stage.assign(text, stage.metal("metal", stage.color(o["color"]), roughness=0.2))
stage.stand_up(text)
bpy.context.view_layer.update()
dim = text.dimensions  # X and Z after standing up
stage.lights_and_world()
stage.camera_fit(dim.x, max(dim.y, dim.z))
stage.swing(text, o["frames"], yaw=o["yaw"])
stage.render(o["name"], o["frames"], o["size"], engine=o["engine"], samples=o["samples"])
