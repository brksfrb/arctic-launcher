"""Turn the plain .glb design prototypes of the mesh cosmetic packs into Arctic mesh cosmetics: wing pairs are
split into a left and a right wing that flap on their own, shoulder and tail pieces get their idle motion, and
the travelling light ("sheen") of each review viewer is written into the file. Writes assets/cosmetics/<id>.glb.

    python tools/make_codex_meshes.py <outputs folder holding aetherglass-wings/, arc-gauntlets/, ...>

Item table below: one entry per cosmetic. The numbers come from each pack's review viewer (orbit.html and
animation.json). The format is documented in docs/content-guide.md ("Sculpted (mesh) cosmetics").
"""
import math
import os
import sys

import numpy as np

from make_mesh_pilots import ASSETS, Glb, mul, primitive, prototype, quat_y, quat_z, subset

TWO_PI = 2 * math.pi


def quat_x(angle):
    return [math.sin(angle / 2), 0.0, 0.0, math.cos(angle / 2)]


def sheen(tint, period, strength=0.4, a=(0, 0, 0), origin=0.0, abs_x=0.0):
    """A travelling light whose place in the sweep is `a . p - origin + abs_x * |x|` (the viewers' formulas)."""
    out = {'period': period, 'tint': tint, 'strength': strength, 'width': 0.065, 'axis': list(a), 'origin': origin, 'length': 1.0}
    if abs_x:
        out['skew'] = {'axis': [1, 0, 0], 'amount': abs_x, 'origin': 0, 'abs': True}
    return out


def rgb(r, g, b):
    return [round(r / 255, 3), round(g / 255, 3), round(b / 255, 3)]


def image_material(png, glb):
    return [{'pbrMetallicRoughness': {'baseColorTexture': {'index': 0}}, 'doubleSided': True}], [{'source': 0}], \
        [{'bufferView': glb.image(png), 'mimeType': 'image/png'}]


def finish(glb, doc, nodes, meshes, png, animation, sheen_json, out_path, has_material):
    doc.update({'asset': {'version': '2.0', 'generator': 'arctic tools/make_codex_meshes.py'}, 'scene': 0, 'nodes': nodes, 'meshes': meshes})
    doc['scenes'] = [{'nodes': [0], 'extras': {'arctic': {'sheen': sheen_json}} if sheen_json else {}}]
    if not doc['scenes'][0]['extras']:
        del doc['scenes'][0]['extras']
    if has_material and png:
        doc['materials'], doc['textures'], doc['images'] = image_material(png, glb)
    if animation:
        doc['animations'] = [animation]
    glb.write(doc, out_path)


def animation_of(glb, length, keys, tracks):
    """tracks: [(node, 'rotation'|'translation', fn(time) -> value)]; one looping animation sampled `keys` times."""
    times = np.array([length * i / keys for i in range(keys + 1)], dtype=np.float32)
    time_acc = glb.add(times, 'SCALAR', 5126, minmax=True)
    samplers, channels = [], []
    for node, path, fn in tracks:
        values = np.array([fn(float(t)) for t in times], dtype=np.float32)
        kind = 'VEC4' if path == 'rotation' else 'VEC3'
        samplers.append({'input': time_acc, 'output': glb.add(values, kind, 5126), 'interpolation': 'LINEAR'})
        channels.append({'sampler': len(samplers) - 1, 'target': {'node': node, 'path': path}})
    return {'name': 'idle', 'samplers': samplers, 'channels': channels}


def wing_pair(source, out_path, sheen_json, flap=7.0, period=3.2, hinge=None):
    """A back piece: the model split into a left and a right wing (nodes under `body`) that flap around the shoulder."""
    prims, png = prototype(source)
    glb = Glb()
    hardware = prims[0]['pos']
    sides = {}
    for name, sign in (('wing_left', 1), ('wing_right', -1)):
        if hinge:
            pivot = [sign * hinge[0], hinge[1], hinge[2]]
        else:
            side = hardware[hardware[:, 0] * sign > 0]
            near = side[np.abs(side[:, 0]) < np.abs(side[:, 0]).min() + 1.5]
            pivot = near.mean(axis=0).round(2).tolist()
        sides[name] = (sign, pivot)
    nodes = [{'name': 'body', 'children': [1, 2]}]
    meshes = []
    textured = any(p['material'] for p in prims)
    for name, (sign, pivot) in sides.items():
        parts = []
        for prim in prims:
            tri_x = prim['pos'][prim['idx'].reshape(-1, 3)].mean(axis=1)[:, 0]
            parts.append(primitive(glb, subset(prim, (tri_x * sign > 0) | ((tri_x == 0) & (sign > 0))), pivot, 0))
        meshes.append({'name': name, 'primitives': parts})
        nodes.append({'name': name, 'mesh': len(meshes) - 1, 'translation': pivot})
    angle = math.radians(flap)
    tracks = []
    for index, (name, (sign, pivot)) in enumerate(sides.items(), start=1):
        def rot(t, sign=sign):
            phase = TWO_PI * t / period
            return mul(quat_z(angle * math.sin(phase) * sign), quat_y(angle * 0.5 * math.sin(phase + 0.9) * -sign))
        tracks.append((index, 'rotation', rot))
    animation = animation_of(glb, period, 16, tracks)
    finish(glb, {}, nodes, meshes, png, animation, sheen_json, out_path, textured)


def single(source, out_path, root, sheen_json, sway=None):
    """A piece that follows one player part (`root`). `sway`: (period, [(pivot, fn(phase) -> quaternion)]) puts the
    whole model on a child node that turns around `pivot`."""
    prims, png = prototype(source)
    glb = Glb()
    textured = any(p['material'] for p in prims)
    animation = None
    if sway:
        period, pivot, fn = sway
        mesh = {'name': root, 'primitives': [primitive(glb, p, pivot, 0) for p in prims]}
        nodes = [{'name': root, 'children': [1]}, {'name': 'sway', 'mesh': 0, 'translation': pivot}]
        animation = animation_of(glb, period, 16, [(1, 'rotation', lambda t: fn(TWO_PI * t / period))])
    else:
        mesh = {'name': root, 'primitives': [primitive(glb, p, [0, 0, 0], 0) for p in prims]}
        nodes = [{'name': root, 'mesh': 0}]
    finish(glb, {}, nodes, [mesh], png, animation, sheen_json, out_path, textured)


def main(folder):
    out = os.path.join(ASSETS, 'cosmetics')

    def src(directory, name):
        return os.path.join(folder, directory, name)

    def dest(name):
        return os.path.join(out, name + '.glb')

    # Wings: the older prototypes are static in their viewers; the flap is Arctic's (as for Helios Wings).
    for directory, file, ident in (
            ('aetherglass-wings', 'aetherglass_wings.glb', 'aetherglass_wings'),
            ('cinder-drake-wings', 'cinder_drake_wings.glb', 'cinder_drake_wings'),
            ('nocturne-wings', 'nocturne_wings.glb', 'nocturne_wings'),
            ('starlace-moth-wings', 'starlace_moth_wings.glb', 'starlace_moth_wings'),
            ('tidebreaker-fins', 'tidebreaker_fins.glb', 'tidebreaker_fins'),
            ('verdant-crown-wings', 'verdant_crown_wings.glb', 'verdant_crown_wings'),
            ('vesper-blade-wings', 'vesper_blade_wings.glb', 'vesper_blade_wings')):
        wing_pair(src(directory, file), dest(ident), None)
    # The two with their own viewer motion and light: hinge (3.1, 22, 3.8), a flap of 0.16 / 0.23 radians in 3.1 s,
    # light running root to tip.
    wing_pair(src('glacier-heron-wings', 'glacier-heron-wings.glb'), dest('glacier_heron_wings'),
              sheen(rgb(220, 242, 255), 3.1, 0.4, (0, 0.013, 0), 0.5166, 1 / 28), flap=math.degrees(0.16), period=3.1, hinge=[3.1, 22.0, 3.8])
    wing_pair(src('vesper-bat-wings', 'vesper-bat-wings.glb'), dest('vesper_bat_wings'),
              sheen(rgb(245, 141, 252), 3.1, 0.4, (0, 0.013, 0), 0.5166, 1 / 28), flap=math.degrees(0.23), period=3.1, hinge=[3.1, 22.0, 3.8])

    # Head pieces
    single(src('amethyst-spire-crown', 'amethyst_spire_crown.glb'), dest('amethyst_spire_crown'), 'head', None)
    single(src('prism-orbit', 'prism_orbit.glb'), dest('prism_orbit'), 'head', None)
    single(src('opaline-antlers', 'opaline_antlers.glb'), dest('opaline_antlers'), 'head',
           sheen(rgb(222, 168, 255), 4.8, 0.4, (0.018, 1 / 12, 0), 32 / 12))
    # Shoulders
    single(src('copper-scarab', 'copper_scarab.glb'), dest('copper_scarab'), 'body', None)
    single(src('tidal-koi', 'tidal_koi.glb'), dest('tidal_koi'), 'body', None)
    # Body: the mantle sways gently from the collar
    single(src('celestial-mantle', 'celestial_mantle.glb'), dest('celestial_mantle'), 'body',
           sheen(rgb(143, 245, 255), 4.2, 0.4, (0.022, -0.1, 0), -2.45),
           sway=(4.2, [0.0, 23.8, 0.0], lambda ph: mul(quat_x(0.04 * math.sin(ph)), quat_z(0.012 * math.sin(ph + 0.4)))))
    # Back pieces
    single(src('moonwell-pack', 'moonwell_pack.glb'), dest('moonwell_pack'), 'body', None)
    single(src('starwell-lantern', 'starwell_lantern.glb'), dest('starwell_lantern'), 'body', None)
    single(src('solar-comet-tail', 'solar_comet_tail.glb'), dest('solar_comet_tail'), 'body',
           sheen(rgb(255, 196, 115), 3.3, 0.4, (0.018, 0, 1 / 17), 3 / 17),
           sway=(3.3, [0.0, 14.0, 3.0], lambda ph: mul(quat_y(0.045 * math.sin(ph)), quat_x(-0.014 * math.sin(ph + 0.7)))))


if __name__ == '__main__':
    main(sys.argv[1])
