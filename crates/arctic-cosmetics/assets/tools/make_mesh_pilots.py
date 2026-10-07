"""Turn the review prototypes (plain .glb meshes) into Arctic mesh cosmetics: the wings are split
into a left and a right wing that move on their own, the fox gets a gentle bob, and both get a
looping idle animation. Writes assets/cosmetics/<id>.glb.

    python tools/make_mesh_pilots.py <folder with helios-sculpt-wings/ and aurora-fox/>

The format is documented in docs/content-guide.md ("Mesh cosmetics").
"""
import json
import math
import os
import struct
import sys

import numpy as np

ASSETS = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def read_glb(path):
    data = open(path, 'rb').read()
    pos, doc, bin_ = 12, None, b''
    while pos < len(data):
        length, kind = struct.unpack('<I4s', data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        if kind == b'JSON':
            doc = json.loads(body)
        else:
            bin_ = body
        pos += 8 + length
    return doc, bin_


def accessor(doc, bin_, index):
    a = doc['accessors'][index]
    view = doc['bufferViews'][a['bufferView']]
    n = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4}[a['type']]
    dtype = {5126: '<f4', 5123: '<u2', 5125: '<u4', 5121: 'u1'}[a['componentType']]
    at = view.get('byteOffset', 0) + a.get('byteOffset', 0)
    out = np.frombuffer(bin_, dtype=dtype, count=a['count'] * n, offset=at).reshape(a['count'], n)
    return out.astype(np.float32) / 255.0 if a.get('normalized') and a['componentType'] == 5121 else out


def prototype(path):
    """Primitives (positions, uvs, colors, triangle indices, has_material) and the first image's PNG bytes."""
    doc, bin_ = read_glb(path)
    prims = []
    for mesh in doc['meshes']:
        for p in mesh['primitives']:
            attrs = p['attributes']
            pos = accessor(doc, bin_, attrs['POSITION']).astype(np.float32)
            uv = accessor(doc, bin_, attrs['TEXCOORD_0']).astype(np.float32) if 'TEXCOORD_0' in attrs else None
            col = accessor(doc, bin_, attrs['COLOR_0']).astype(np.float32) if 'COLOR_0' in attrs else None
            idx = accessor(doc, bin_, p['indices'])[:, 0].astype(np.uint32)
            prims.append({'pos': pos, 'uv': uv, 'col': col, 'idx': idx, 'material': 'material' in p})
    png = None
    if doc.get('images'):
        view = doc['bufferViews'][doc['images'][0]['bufferView']]
        at = view.get('byteOffset', 0)
        png = bin_[at:at + view['byteLength']]
    return prims, png


def subset(prim, keep_tris):
    """The triangles `keep_tris` (a boolean per triangle) as a primitive of their own, vertices renumbered."""
    tris = prim['idx'].reshape(-1, 3)[keep_tris]
    used, inverse = np.unique(tris.ravel(), return_inverse=True)
    return {
        'pos': prim['pos'][used],
        'uv': None if prim['uv'] is None else prim['uv'][used],
        'col': None if prim['col'] is None else prim['col'][used],
        'idx': inverse.astype(np.uint32),
        'material': prim['material'],
    }


class Glb:
    def __init__(self):
        self.bin = bytearray()
        self.views, self.accessors = [], []

    def _view(self, data, target=None):
        while len(self.bin) % 4:
            self.bin.append(0)
        view = {'buffer': 0, 'byteOffset': len(self.bin), 'byteLength': len(data)}
        if target:
            view['target'] = target
        self.bin += data
        self.views.append(view)
        return len(self.views) - 1

    def add(self, array, kind, component, normalized=False, minmax=False):
        data = np.ascontiguousarray(array)
        a = {'bufferView': self._view(data.tobytes()), 'componentType': component, 'count': int(data.shape[0]), 'type': kind}
        if normalized:
            a['normalized'] = True
        if minmax:
            flat = data.reshape(data.shape[0], -1)
            a['min'] = [float(x) for x in flat.min(axis=0)]
            a['max'] = [float(x) for x in flat.max(axis=0)]
        self.accessors.append(a)
        return len(self.accessors) - 1

    def image(self, png):
        return self._view(png)

    def write(self, doc, path):
        doc['bufferViews'] = self.views
        doc['accessors'] = self.accessors
        doc['buffers'] = [{'byteLength': len(self.bin)}]
        js = json.dumps(doc, separators=(',', ':')).encode()
        js += b' ' * (-len(js) % 4)
        bn = bytes(self.bin) + b'\0' * (-len(self.bin) % 4)
        total = 12 + 8 + len(js) + 8 + len(bn)
        with open(path, 'wb') as f:
            f.write(struct.pack('<4sII', b'glTF', 2, total))
            f.write(struct.pack('<I4s', len(js), b'JSON') + js)
            f.write(struct.pack('<I4s', len(bn), b'BIN\0') + bn)
        print(f'{os.path.basename(path)}: {total // 1024} KB')


def primitive(glb, prim, offset, material_index):
    pos = prim['pos'] - np.array(offset, dtype=np.float32)
    attrs = {'POSITION': glb.add(pos, 'VEC3', 5126, minmax=True)}
    if prim['uv'] is not None:
        attrs['TEXCOORD_0'] = glb.add(prim['uv'], 'VEC2', 5126)
    if prim['col'] is not None:
        attrs['COLOR_0'] = glb.add((np.clip(prim['col'], 0, 1) * 255).round().astype(np.uint8), 'VEC4', 5121, normalized=True)
    out = {'attributes': attrs, 'indices': glb.add(prim['idx'].astype(np.uint32), 'SCALAR', 5125)}
    if prim['material'] and material_index is not None:
        out['material'] = material_index
    return out


def quat_z(angle):
    return [0.0, 0.0, math.sin(angle / 2), math.cos(angle / 2)]


def quat_y(angle):
    return [0.0, math.sin(angle / 2), 0.0, math.cos(angle / 2)]


def mul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz]


def wings(source, out_path):
    prims, png = prototype(source)
    glb = Glb()
    sides = {}
    for name, sign in (('wing_left', 1), ('wing_right', -1)):
        # the shoulder: where this side's metalwork comes closest to the middle
        hardware = prims[0]['pos']
        side = hardware[hardware[:, 0] * sign > 0]
        near = side[np.abs(side[:, 0]) < np.abs(side[:, 0]).min() + 1.5]
        pivot = near.mean(axis=0).round(2).tolist()
        sides[name] = (sign, pivot)
    nodes = [{'name': 'body', 'children': [1, 2]}]
    meshes = []
    for name, (sign, pivot) in sides.items():
        mesh_prims = []
        for prim in prims:
            tri_x = prim['pos'][prim['idx'].reshape(-1, 3)].mean(axis=1)[:, 0]
            mesh_prims.append(primitive(glb, subset(prim, tri_x * sign > 0), pivot, 0))
        meshes.append({'name': name, 'primitives': mesh_prims})
        nodes.append({'name': name, 'mesh': len(meshes) - 1, 'translation': pivot})
    # one slow flap with a little sway: 3.2 s, 16 keys
    length, keys = 3.2, 16
    times = np.array([length * i / keys for i in range(keys + 1)], dtype=np.float32)
    samplers, channels = [], []
    time_acc = glb.add(times, 'SCALAR', 5126, minmax=True)
    for node_index, (name, (sign, pivot)) in enumerate(sides.items(), start=1):
        rotations = []
        for t in times:
            phase = 2 * math.pi * t / length
            flap = math.radians(7.0) * math.sin(phase) * sign
            sway = math.radians(3.5) * math.sin(phase + 0.9) * -sign
            rotations.append(mul(quat_z(flap), quat_y(sway)))
        samplers.append({'input': time_acc, 'output': glb.add(np.array(rotations, dtype=np.float32), 'VEC4', 5126), 'interpolation': 'LINEAR'})
        channels.append({'sampler': len(samplers) - 1, 'target': {'node': node_index, 'path': 'rotation'}})
    doc = {
        'asset': {'version': '2.0', 'generator': 'arctic tools/make_mesh_pilots.py'},
        'scene': 0, 'scenes': [{'nodes': [0]}], 'nodes': nodes, 'meshes': meshes,
        'materials': [{'pbrMetallicRoughness': {'baseColorTexture': {'index': 0}}, 'doubleSided': True}],
        'textures': [{'source': 0}],
        'images': [{'bufferView': glb.image(png), 'mimeType': 'image/png'}],
        'animations': [{'name': 'idle', 'samplers': samplers, 'channels': channels}],
    }
    glb.write(doc, out_path)


def shoulder_pet(source, out_path):
    prims, _ = prototype(source)
    glb = Glb()
    mesh = {'name': 'fox', 'primitives': [primitive(glb, prims[0], [0, 0, 0], None)]}
    length, keys = 2.4, 12
    times = np.array([length * i / keys for i in range(keys + 1)], dtype=np.float32)
    bob = np.array([[0.0, 0.45 * math.sin(2 * math.pi * t / length), 0.0] for t in times], dtype=np.float32)
    time_acc = glb.add(times, 'SCALAR', 5126, minmax=True)
    doc = {
        'asset': {'version': '2.0', 'generator': 'arctic tools/make_mesh_pilots.py'},
        'scene': 0, 'scenes': [{'nodes': [0]}], 'nodes': [{'name': 'body', 'mesh': 0}], 'meshes': [mesh],
        'animations': [{'name': 'idle', 'samplers': [{'input': time_acc, 'output': glb.add(bob, 'VEC3', 5126), 'interpolation': 'LINEAR'}],
                        'channels': [{'sampler': 0, 'target': {'node': 0, 'path': 'translation'}}]}],
    }
    glb.write(doc, out_path)


if __name__ == '__main__':
    src = sys.argv[1]
    out = os.path.join(ASSETS, 'cosmetics')
    wings(os.path.join(src, 'helios-sculpt-wings', 'helios_sculpt.glb'), os.path.join(out, 'helios_wings.glb'))
    shoulder_pet(os.path.join(src, 'aurora-fox', 'aurora_fox.glb'), os.path.join(out, 'aurora_fox.glb'))
