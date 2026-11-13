"""Package Mixin diagnostics without changing unrelated shipped client bytecode.

First: python mod/tools/diagnostics/mixin_overlay.py BASELINE.pack --extract
Then build the 1.18+ targets with mod/build.py (the extracted older jars stay intact).
Finally: python mod/tools/diagnostics/mixin_overlay.py BASELINE.pack
All entries outside the startup diagnostics allowlist must match BASELINE exactly.
"""
import importlib.util
import io
import json
import lzma
from pathlib import Path
import struct
import sys
import zipfile

MOD = Path(__file__).resolve().parents[2]
STARTUP = 'com/arcticlauncher/mod/startup/'
CLASSES = ('ArcticAgent', 'MixinReplay', 'Preloader', 'MixinFailureDiagnostics')


def allowed(name):
    return any(name == STARTUP + cls + '.class' or
               name.startswith(STARTUP + cls + '$') and name.endswith('.class')
               for cls in CLASSES)


def unpack(path):
    raw = lzma.decompress(Path(path).read_bytes(), format=lzma.FORMAT_ALONE)
    magic = b'ARCTICPACK1\n'
    assert raw.startswith(magic)
    offset = len(magic)
    size = struct.unpack_from('<I', raw, offset)[0]
    offset += 4
    index = json.loads(raw[offset:offset + size])
    offset += size
    files = []
    for size in index['sizes']:
        files.append(raw[offset:offset + size])
        offset += size
    assert offset == len(raw)
    return {version: {name: files[number] if number >= 0 else b'' for name, number in entries}
            for version, entries in index['jars'].items()}


def jar_path(version):
    return MOD / 'dist' / (f'arctic-{version}.jar' if version.startswith('forge-')
                           else f'arctic-mod-{version}.jar')


def jar_bytes(entries):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, 'w') as jar:
        for name, data in entries.items():
            entry = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            jar.writestr(entry, data)
    return buffer.getvalue()


def main():
    baseline = unpack(sys.argv[1])
    (MOD / 'dist').mkdir(exist_ok=True)
    extract = '--extract' in sys.argv
    for version, old in baseline.items():
        path = jar_path(version)
        if extract:
            path.write_bytes(jar_bytes(old))
            continue
        updated = dict(old)
        if STARTUP + 'ArcticAgent.class' in old:
            with zipfile.ZipFile(path) as fresh:
                for name in fresh.namelist():
                    if allowed(name):
                        updated[name] = fresh.read(name)
            assert STARTUP + 'MixinFailureDiagnostics.class' in updated, version
            # Don't let a previously overlaid jar stand in for a fresh compiler result.
            assert b'arctic$transformWithDiagnostics' in updated[STARTUP + 'ArcticAgent.class'], version
        changed = {name for name, data in updated.items() if name not in old or old[name] != data}
        assert set(old) <= set(updated)
        assert all(allowed(name) for name in changed), (version, changed)
        path.write_bytes(jar_bytes(updated))
        print(f'{version}: {len(changed)} startup diagnostics entries; other entries match baseline', flush=True)
    if extract:
        return
    spec = importlib.util.spec_from_file_location('arctic_build', MOD / 'build.py')
    build = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(build)
    build.pack(json.loads((MOD / 'targets.json').read_text()))
    packed = unpack(MOD / 'dist' / 'arctic-client.pack')
    assert set(packed) == set(baseline)
    for version, old in baseline.items():
        assert set(old) <= set(packed[version])
        assert all(allowed(name) or old.get(name) == data for name, data in packed[version].items())


if __name__ == '__main__':
    main()
