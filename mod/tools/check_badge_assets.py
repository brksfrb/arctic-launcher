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
        names = jar.namelist()
        for retired in ['badge1', 'badge2', 'badge3', 'badge_shield', 'badge_shield_tab', 'badge_shield_tag']:
            assert not any(Path(n).stem == retired for n in names), (path, retired)
        assert not any(Path(n).stem.split('$')[0] in {'BadgeTest', 'LegacyBadgeTest'} for n in names), path
        for picture in sorted((ASSETS / 'assets/arctic').glob('badge*.png')):
            asset = f'assets/arctic/{picture.name}'
            assert jar.read(asset) == picture.read_bytes(), (path, asset)
        for source in sorted((ASSETS / 'assets/arctic/font').glob('badge*.json')):
            font = f'assets/arctic/font/{source.name}'
            assert jar.read(font) == source.read_bytes(), (path, font)
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
