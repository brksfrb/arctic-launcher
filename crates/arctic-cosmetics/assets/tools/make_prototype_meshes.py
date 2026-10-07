"""Turn the two front-visible review prototypes (plain vertex-colored .glb meshes) into Arctic mesh cosmetics:
the gauntlets are split into a left and a right half that follow the player's arms, the visor follows the head.
Writes assets/cosmetics/<id>.glb.

    python tools/make_prototype_meshes.py <folder with arc-gauntlets/ and aurora-visor/>
"""
import os
import sys

import numpy as np

from make_mesh_pilots import ASSETS, Glb, primitive, prototype, subset


def write(prims, nodes_meshes, out_path, sheen):
    glb = Glb()
    meshes, nodes = [], []
    for name, keep in nodes_meshes:
        mesh_prims = [primitive(glb, subset(p, keep(p)) if keep else p, [0, 0, 0], None) for p in prims]
        meshes.append({'name': name, 'primitives': mesh_prims})
        nodes.append({'name': name, 'mesh': len(meshes) - 1})
    doc = {
        'asset': {'version': '2.0', 'generator': 'arctic tools/make_prototype_meshes.py'},
        'scene': 0, 'scenes': [{'nodes': list(range(len(nodes))), 'extras': {'arctic': {'sheen': sheen}}}],
        'nodes': nodes, 'meshes': meshes,
    }
    glb.write(doc, out_path)


# The review viewer's travelling light, in the file's own units (see docs/content-guide.md, "Sheen").
TINT = [0.63, 0.96, 1.0]
GAUNTLET_SHEEN = {'period': 3.0, 'tint': TINT, 'strength': 0.4, 'width': 0.065, 'axis': [0, -1, 0], 'origin': -19.4, 'length': 7.2,
                  'skew': {'axis': [1, 0, 0], 'amount': 0.018, 'origin': 0, 'abs': True}}
VISOR_SHEEN = {'period': 3.0, 'tint': TINT, 'strength': 0.4, 'width': 0.065, 'axis': [1, 0, 0], 'origin': -4.1, 'length': 8.2,
               'skew': {'axis': [0, 1, 0], 'amount': 0.025, 'origin': 29, 'abs': False}}


def centroid_x(prim):
    return prim['pos'][prim['idx'].reshape(-1, 3)].mean(axis=1)[:, 0]


def main(folder):
    prims, _ = prototype(os.path.join(folder, 'arc-gauntlets', 'arc_gauntlets.glb'))
    # +x is the player's left
    write(prims, [('rightArm', lambda p: centroid_x(p) < 0), ('leftArm', lambda p: centroid_x(p) >= 0)],
          os.path.join(ASSETS, 'cosmetics', 'arc_gauntlets.glb'), GAUNTLET_SHEEN)
    prims, _ = prototype(os.path.join(folder, 'aurora-visor', 'aurora_visor.glb'))
    write(prims, [('head', None)], os.path.join(ASSETS, 'cosmetics', 'aurora_visor.glb'), VISOR_SHEEN)


if __name__ == '__main__':
    main(sys.argv[1])
