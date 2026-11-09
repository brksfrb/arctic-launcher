"""Check every built mod's badge resources, including the standalone modern client pack.

Run after mod/build.py. Uses only Python's standard library.
"""
import json
import zipfile
from pathlib import Path

MOD = Path(__file__).resolve().parents[1]
ASSETS = MOD / 'core/src/main/resources'


def check(path):
    with zipfile.ZipFile(path) as jar:
        # Legacy draws badge*.png directly; modern versions use the bitmap fonts.
        for picture in sorted((ASSETS / 'assets/arctic').glob('badge*.png')):
            name = picture.stem
            asset = f'assets/arctic/{name}.png'
            assert jar.read(asset) == (ASSETS / asset).read_bytes(), (path, asset)
            font = f'assets/arctic/font/{name}.json'
            assert jar.read(font) == (ASSETS / font).read_bytes(), (path, font)
            prefixes = ['']
            # These versions register a required client pack without depending on Fabric API.
            if 'resourcepacks/client/pack.mcmeta' in jar.namelist():
                prefixes.append('resourcepacks/client/')
            for prefix in prefixes:
                assert jar.read(prefix + font) == jar.read(font), (path, prefix + font)
                for provider in json.loads(jar.read(font))['providers']:
                    if provider['type'] == 'bitmap':
                        namespace, picture = provider['file'].split(':')
                        texture = f'assets/{namespace}/textures/{picture}'
                        assert jar.read(prefix + texture) == (ASSETS / texture).read_bytes(), (path, prefix + texture)
    print('PASS', path.name)


def main():
    jars = sorted((MOD / 'dist').glob('arctic-mod-*.jar'))
    forge = MOD / 'dist/arctic-forge-1.8.9.jar'
    if forge.exists():
        jars.append(forge)
    assert len(jars) == len(json.loads((MOD / 'targets.json').read_text())) + 1, 'Build all targets and Forge first'
    for jar in jars:
        check(jar)
    print(f'All {len(jars)} jars have current badge assets in every applicable resource path')


if __name__ == '__main__':
    main()
