"""Loopback-only assets for an isolated installed-game attachment test.

The service supplies a diagnostic hat through the ordinary Arctic protocol.
It never supplies an engine frame or claims feature parity.
"""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from io import BytesIO
import json
from pathlib import Path
import time
from urllib.parse import parse_qs, urlsplit

from PIL import Image

ITEM = "arctic_probe_hat"


def assets(cape=False, skin=False, emote=False):
    geometry = {"minecraft:geometry": [{"description": {"texture_width": 64, "texture_height": 64},
        "bones": [{"name": "head", "pivot": [0, 24, 0], "cubes": [
            {"origin": [-6, 31, -6], "size": [12, 4, 12], "uv": [0, 0]}]}]}]}
    model = json.dumps(geometry, separators=(",", ":")).encode()
    stream = BytesIO()
    Image.new("RGBA", (64, 64), (255, 0, 255, 255)).save(stream, format="PNG")
    texture = stream.getvalue()
    model_hash = hashlib.sha1(model).hexdigest()
    texture_hash = hashlib.sha1(texture).hexdigest()
    catalog = {"cosmetics": [{"id": ITEM, "name": "Native attachment probe", "slot": "hat",
        "model": model_hash, "texture": texture_hash}], "meshes": [], "emotes": []}
    files = {f"/v1/assets/{model_hash}": (model, "application/json"),
        f"/v1/textures/{texture_hash}.png": (texture, "image/png")}
    if emote:
        animation=json.dumps({"animations":{"animation.arctic_probe":{"loop":True,"animation_length":2,
            "bones":{"leftArm":{"rotation":{"0":[0,0,0],"1":[0,0,-120],"2":[0,0,0]}},
                     "rightArm":{"rotation":{"0":[0,0,0],"1":[0,0,120],"2":[0,0,0]}}}}}},separators=(",",":")).encode()
        animation_hash=hashlib.sha1(animation).hexdigest()
        catalog["emotes"].append({"id":"arctic_probe_emote","name":"Native emote probe","animation":animation_hash})
        files[f"/v1/assets/{animation_hash}"]=(animation,"application/json")
    cape_hash = None
    if cape:
        strip = Image.new("RGBA", (64, 64), (255, 0, 0, 255))
        strip.paste((0, 255, 0, 255), (0, 32, 64, 64))
        stream = BytesIO()
        strip.save(stream, format="PNG")
        png = stream.getvalue()
        cape_hash = hashlib.sha1(png).hexdigest()
        files[f"/v1/textures/{cape_hash}.png"] = (png, "image/png")
    skin_hash = None
    if skin:
        stream = BytesIO()
        Image.new("RGBA", (64, 64), (0, 80, 255, 255)).save(stream, format="PNG")
        png = stream.getvalue()
        skin_hash = hashlib.sha1(png).hexdigest()
        files[f"/v1/textures/{skin_hash}.png"] = (png, "image/png")
    return catalog, files, cape_hash, skin_hash


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--seconds", type=int, default=900)
    parser.add_argument("--cape", action="store_true")
    parser.add_argument("--skin", action="store_true")
    parser.add_argument("--emote", action="store_true")
    args = parser.parse_args()
    root = args.directory.resolve(strict=True)
    config = json.loads((root / "arctic-offline-probe.json").read_text(encoding="utf-8-sig"))
    if (config.get("mode") != "loopback-and-deny-user-provider"
            or config.get("empty_user_provider") is not True or config.get("auto_world") is not True):
        raise ValueError("cosmetic service requires the isolated automatic offline probe")
    catalog, files, cape_hash, skin_hash = assets(args.cape, args.skin,args.emote)
    log = root / "cosmetics-service.jsonl"

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            url = urlsplit(self.path)
            with log.open("a", encoding="utf-8") as out:
                out.write(json.dumps({"path": url.path, "query": url.query}) + "\n")
            if url.path in files:
                body, kind = files[url.path]
            else:
                kind = "application/json"
                if url.path == "/v1/players":
                    uuids = parse_qs(url.query).get("uuids", [""])[0].split(",")
                    value = {uuid: {"arctic": True, "cosmetics": [ITEM]} for uuid in uuids
                             if len(uuid) == 32 and all(c in "0123456789abcdef" for c in uuid)}
                    if cape_hash:
                        for look in value.values():
                            look["cape"] = cape_hash
                    if skin_hash:
                        for look in value.values():
                            look.update(skin=skin_hash, slim=False)
                elif url.path == "/v1/cosmetics":
                    value = catalog
                elif url.path == "/v1/emotes":
                    value = {}
                elif url.path == "/v1/capes":
                    value = []
                else:
                    self.send_error(404)
                    return
                body = json.dumps(value).encode()
            self.send_response(200)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    server.daemon_threads = True
    server.timeout = .25
    ready = root / "cosmetics-service.json.tmp"
    ready.write_text(json.dumps({"schema": 1,
        "url": f"http://127.0.0.1:{server.server_port}", "item": ITEM,
        "cape": {"hash": cape_hash, "frames": 2, "fps": 8} if cape_hash else None,
        "skin": {"hash": skin_hash, "slim": False} if skin_hash else None}), encoding="utf-8")
    ready.replace(root / "cosmetics-service.json")
    deadline = time.monotonic() + args.seconds
    try:
        while time.monotonic() < deadline and not (root / "report.json").exists():
            server.handle_request()
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
