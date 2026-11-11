"""Build the native Windows Bedrock client and its shared Java core bundle."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import re


def decode_capture(ffmpeg, ffprobe, video, width, height, frames):
    if ffprobe:
        probe = json.loads(subprocess.check_output([str(ffprobe), "-v", "error", "-count_frames", "-show_streams", "-of", "json", str(video)], timeout=30))
        stream = next(s for s in probe["streams"] if s["codec_type"] == "video")
        assert (stream["codec_name"], stream["width"], stream["height"], int(stream["nb_read_frames"])) == ("h264", width, height, frames), stream
    pixels = subprocess.check_output([str(ffmpeg), "-v", "error", "-i", str(video), "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"], timeout=30)
    assert len(pixels) == width * height * 4 * frames, len(pixels)
    return pixels[:width * height * 4]


def check_dx12_overlay(pixels):
    # The host clears only red/green. The shared Java HUD/footer adds bright
    # pixels; checking them catches an obsolete overlay texture after resizing.
    bright = sum(min(pixels[i:i+3]) > 140 for i in range(0, len(pixels), 4))
    assert bright > 500, ("DX12 shared-client overlay disappeared", bright)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java-home", default=os.environ.get("JAVA_HOME"))
    parser.add_argument("--config-dir", type=Path)
    parser.add_argument("--java-reference-jar", type=Path, help="optional local Java 26.2 archive for pinned reference artwork; asset access only")
    parser.add_argument("--game-version", required=True)
    parser.add_argument("--offline-name", default="ArcticGuest")
    parser.add_argument("--renderer", choices=["dx11", "dx12"], default="dx12")
    parser.add_argument("--release", action="store_true")
    parser.add_argument("--smoke", action="store_true", help="also inject into a disposable hidden D3D11 host")
    parser.add_argument("--capture-smoke", action="store_true", help="also encode/decode real GPU frames as MP4 (requires FFmpeg; ffprobe optional)")
    parser.add_argument("--ffmpeg", type=Path)
    parser.add_argument("--ffprobe", type=Path)
    args = parser.parse_args()
    if args.capture_smoke:
        args.smoke = True
        args.ffmpeg = args.ffmpeg or shutil.which("ffmpeg")
        args.ffprobe = args.ffprobe or shutil.which("ffprobe")
        if not args.ffmpeg:
            parser.error("capture smoke needs --ffmpeg or FFmpeg on PATH")
    if not re.fullmatch(r"[A-Za-z0-9_]{3,16}", args.offline_name):
        parser.error("offline name needs 3–16 letters, digits or underscores")
    if os.name != "nt":
        parser.error("the native Bedrock adapter requires Windows x64")
    if not args.java_home:
        parser.error("set JAVA_HOME or pass --java-home with a JDK (17 or newer)")
    java = Path(args.java_home).resolve()
    reference = args.java_reference_jar.resolve() if args.java_reference_jar else None
    if reference is not None and not reference.is_file():
        parser.error("Java reference archive must be a local file")
    if not (java / "bin/server/jvm.dll").is_file():
        parser.error("Java home must contain bin/server/jvm.dll")
    adapter = Path(__file__).resolve().parent
    repo = adapter.parents[2]
    env = dict(os.environ, JAVA_HOME=str(java))
    subprocess.run([str(repo / "mod/gradlew.bat"), "-p", str(adapter), ":test", ":bundle", ":smoke"],
                   cwd=repo, env=env, check=True)
    command = ["cargo", "build", "--manifest-path", str(adapter / "native/Cargo.toml"), "--all-targets", "--locked"]
    if args.release:
        command.append("--release")
    subprocess.run(command, cwd=repo, check=True)
    dist = adapter / "dist"
    dist.mkdir(exist_ok=True)
    target = adapter / "native/target" / ("release" if args.release else "debug")
    for name in ["arctic_bedrock.dll", "arctic-bedrock-loader.exe"]:
        shutil.copy2(target / name, dist / name)
    shutil.copy2(adapter / "build/libs/arctic-bedrock.jar", dist / "arctic-bedrock.jar")
    (dist / "include").mkdir(exist_ok=True)
    shutil.copy2(adapter / "native/include/arctic_bedrock_engine.h", dist / "include/arctic_bedrock_engine.h")
    shutil.copy2(adapter / "native/ENGINE.md", dist / "ENGINE.md")
    licenses = dist / "licenses"
    licenses.mkdir(exist_ok=True)
    for source, name in [
        (repo / "LICENSE", "Arctic-GPL-3.0.txt"),
        (adapter / "native/vendor/hudhook/LICENSE", "hudhook-MIT.txt"),
        (adapter / "native/vendor/hudhook/vendor/minhook/LICENSE.txt", "MinHook-license.txt"),
        (adapter / "native/THIRD_PARTY.md", "THIRD_PARTY.md"),
    ]:
        shutil.copy2(source, licenses / name)
    config = {
        "java_home": str(java),
        "config_dir": str((args.config_dir or dist / "game/config").resolve()),
        "game_version": args.game_version,
        "offline_name": args.offline_name,
        "renderer": args.renderer,
    }
    config_file = dist / "arctic-bedrock.json"
    if reference is not None:
        config["java_reference_jar"] = str(reference)
    config_file.write_text(json.dumps(config, indent=2) + "\n", encoding="utf-8")
    if args.smoke:
        # No saved launcher account or client session enters the headless host.
        with tempfile.TemporaryDirectory(prefix="arctic-bedrock-offline-") as temp:
            smoke = Path(temp)
            for name in ["arctic_bedrock.dll", "arctic-bedrock-loader.exe", "arctic-bedrock.jar"]:
                shutil.copy2(dist / name, smoke / name)
            smoke_config = dict(config, renderer="dx11", offline_name="ArcticTest", config_dir=str(smoke / "config"))
            (smoke / "arctic-bedrock.json").write_text(json.dumps(smoke_config), encoding="utf-8")
            command = [str(target / "examples/render_host.exe"), str(smoke)]
            video = smoke / "native-capture.mp4"
            if args.capture_smoke:
                command += [str(Path(args.ffmpeg).resolve()), str(video)]
            subprocess.run(command, cwd=repo, check=True, timeout=60)
            if args.capture_smoke:
                pixels = decode_capture(args.ffmpeg, args.ffprobe, video, 1600, 900, 8)
                blue = red = 0
                for y in range(820):
                    for x in range(800, 1600):
                        r, g, b = pixels[(y*1600+x)*4:(y*1600+x)*4+3]
                        blue += b > 80 and b > r*2
                        red += r > 80 and r > b*2
                assert blue > 5000 and red > 5000, (blue, red)
                print(f"MP4 decode passed: H.264, 1600x900, 8 frames, skin pixels {blue}, cape pixels {red}")
                smoke_config["renderer"] = "dx12"
                (smoke / "arctic-bedrock.json").write_text(json.dumps(smoke_config), encoding="utf-8")
                dx12_video = smoke / "native-dx12.mp4"
                subprocess.run([str(target / "examples/render_host_dx12.exe"), str(smoke), str(Path(args.ffmpeg).resolve()), str(dx12_video)],
                               cwd=repo, check=True, timeout=60)
                pixels = decode_capture(args.ffmpeg, args.ffprobe, dx12_video, 318, 178, 8)
                check_dx12_overlay(pixels)
                left = pixels[(10*318+10)*4:(10*318+10)*4+3]
                right = pixels[(10*318+300)*4:(10*318+300)*4+3]
                assert left[1] > 100 and left[0] < 70 and right[0] > 100 and right[1] < 70, (left, right)
                print("DX12 MP4 decode passed: BGRA channel order, padded GPU row pitch, H.264, 318x178, 8 frames")
                for phase, width, height in [(1,320,180),(2,318,178),(3,334,188)]:
                    pixels=decode_capture(args.ffmpeg,args.ffprobe,Path(str(dx12_video)+f".phase{phase}.mp4"),width,height,8)
                    check_dx12_overlay(pixels)
                    left=pixels[(10*width+10)*4:(10*width+10)*4+3]
                    right=pixels[(10*width+width-18)*4:(10*width+width-18)*4+3]
                    assert left[1]>100 and left[0]<70 and right[0]>100 and right[1]<70,(phase,left,right)
                print("DX12 MP4 decode passed after size-only resize, format/buffer-count resize and same-window swap-chain replacement")
    print(f"Native preview bundle: {dist}")
    print("Offline Arctic profile enabled. Exact-build world observers are available; complete in-world cosmetics bindings remain pending.")


if __name__ == "__main__":
    main()
