#!/usr/bin/env python3
"""stresspack.py <client.jar> <out_dir> <pack_format> [size]
Nearest-neighbour upscale of every block texture to <size> px per frame (default 128), keeping
animation .mcmeta files: an atlas/decode stress fixture standing in for a large HD pack."""
import io, json, sys, zipfile
from pathlib import Path
from PIL import Image

jar, out, fmt = sys.argv[1], Path(sys.argv[2]), int(sys.argv[3])
size = int(sys.argv[4]) if len(sys.argv) > 4 else 128
out.mkdir(parents=True, exist_ok=True)
(out / 'pack.mcmeta').write_text(json.dumps({'pack': {'pack_format': fmt, 'description': f'{size}px stress'}}))
count = 0
with zipfile.ZipFile(jar) as z:
    for n in z.namelist():
        if not n.startswith('assets/minecraft/textures/block/'):
            continue
        f = out / n
        f.parent.mkdir(parents=True, exist_ok=True)
        if n.endswith('.png'):
            im = Image.open(io.BytesIO(z.read(n)))
            scale = max(1, size // im.size[0])
            im.resize((im.size[0] * scale, im.size[1] * scale), Image.Resampling.NEAREST).save(f, compress_level=6)
            count += 1
        elif n.endswith('.mcmeta'):
            f.write_bytes(z.read(n))
print(count, 'textures')
