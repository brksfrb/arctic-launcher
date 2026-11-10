#!/usr/bin/env python3
"""Reconstruct client jars from the launcher's tracked ARCTICPACK1 release artifact."""
import argparse
import json
import lzma
from pathlib import Path
import re
import struct
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pack', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--version')
    args = parser.parse_args()
    raw = lzma.decompress(args.pack.read_bytes(), format=lzma.FORMAT_ALONE)
    magic = b'ARCTICPACK1\n'
    if not raw.startswith(magic):
        raise SystemExit('Invalid pack magic')
    pos = len(magic)
    size = struct.unpack_from('<I', raw, pos)[0]
    pos += 4
    index = json.loads(raw[pos:pos + size])
    pos += size
    blobs = []
    for size in index['sizes']:
        blobs.append(raw[pos:pos + size])
        pos += size
    if pos != len(raw):
        raise SystemExit('Pack length does not match index')
    args.output.mkdir(parents=True, exist_ok=True)
    for version, entries in index['jars'].items():
        if args.version and version != args.version:
            continue
        if not re.fullmatch(r'(forge-)?[0-9.]+', version):
            raise SystemExit('Unexpected version key: ' + version)
        name = 'arctic-' + version if version.startswith('forge-') else 'arctic-mod-' + version
        target = args.output / (name + '.jar')
        with zipfile.ZipFile(target, 'w', zipfile.ZIP_DEFLATED) as jar:
            for name, number in entries:
                jar.writestr(name, blobs[number] if number >= 0 else b'')
        print(target)


if __name__ == '__main__':
    main()
