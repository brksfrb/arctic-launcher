"""Rebuild the diagnostics pack from a shipped baseline and freshly built target jars.

Run mod/build.py first, then (JDK 25 on JAVA_HOME):
    python mod/tools/diagnostics/overlay.py BASELINE.pack
Only diagnostics classes/methods change; every other packed entry is byte-identical.
"""
import importlib.util
import json
import lzma
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import zipfile

MOD = Path(__file__).resolve().parents[2]
HELPER = Path(__file__).with_name('BaselineOverlay.java')


def main():
    raw = lzma.decompress(Path(sys.argv[1]).read_bytes(), format=lzma.FORMAT_ALONE)
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
    java = Path(os.environ['JAVA_HOME']) / 'bin'
    cache = Path(os.environ.get('GRADLE_USER_HOME', Path.home() / '.gradle'))
    asm = os.pathsep.join(str(next((cache / 'caches/modules-2/files-2.1/org.ow2.asm' / artifact / '9.9')
                                  .glob('*/*.jar'))) for artifact in ('asm', 'asm-tree'))
    with tempfile.TemporaryDirectory(prefix='arctic-diagnostics-') as temporary:
        root = Path(temporary)
        subprocess.run([str(java / 'javac'), '-cp', asm, '-d', str(root), str(HELPER)], check=True)
        for version, entries in index['jars'].items():
            baseline = root / 'baseline.jar'
            output = root / 'output.jar'
            jar = MOD / 'dist' / (f'arctic-{version}.jar' if version.startswith('forge-')
                                  else f'arctic-mod-{version}.jar')
            with zipfile.ZipFile(baseline, 'w') as archive:
                for name, number in entries:
                    archive.writestr(name, files[number] if number >= 0 else b'')
            subprocess.run([str(java / 'java'), '-cp', str(root) + os.pathsep + asm, 'BaselineOverlay',
                            str(baseline), str(jar), str(output)], check=True)
            # Verify the complete entry allowlist against main before publishing the pack.
            with zipfile.ZipFile(baseline) as old, zipfile.ZipFile(output) as new:
                changed = [name for name in new.namelist()
                           if name not in old.namelist() or new.read(name) != old.read(name)]
                allowed = {'com/arcticlauncher/client/ArcticClient.class',
                           'com/arcticlauncher/mod/startup/ExitTrace.class',
                           'com/arcticlauncher/mod/mixin/ReplayFrameMixin.class',
                           'com/arcticlauncher/legacy/mixin/MinecraftClientReplayMixin.class'}
                assert set(old.namelist()) <= set(new.namelist())
                assert all(b'arctic.diagnostics.test' not in new.read(name)
                           for name in new.namelist() if name.endswith('.class'))
                assert all(name in allowed or name.startswith('com/arcticlauncher/client/CrashDiagnostics')
                           for name in changed), changed
                print(f'{version}: {len(changed)} diagnostics entries; all other entries match baseline', flush=True)
            jar.write_bytes(output.read_bytes())
    spec = importlib.util.spec_from_file_location('arctic_build', MOD / 'build.py')
    build = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(build)
    build.pack(json.loads((MOD / 'targets.json').read_text()))


if __name__ == '__main__':
    main()
