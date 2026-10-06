"""Write the motion blur post effects into the Arctic Client's built-in pack (26.1 and later).

Usage: python mod/tools/motion_blur.py
Writes mod/versions/fabric/src/main/resources/resourcepacks/client/assets/arctic/...

Each frame is blended with the last blended frame, kept in a persistent target:
out = mix(now, previous, strength), previous = out. One post effect per strength
(post-effect uniforms are fixed in the JSON) and per shader dialect: 26.3's
shaders are Vulkan-style GLSL, 26.1 and 26.2 plain GLSL 330.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "versions/fabric/src/main/resources/resourcepacks/client"
ASSETS = ROOT / "assets/arctic"

# Strength 1 (Low) to 4 (Max): how much of the previous frame stays.
STRENGTHS = {1: 0.5, 2: 0.65, 3: 0.78, 4: 0.88}

# Shader dialect -> (fragment shader name, shader source).
DIALECTS = {
    "": ("motion_blur", """#version 330
#extension GL_ARB_separate_shader_objects : require

uniform sampler2D InSampler;
uniform sampler2D PrevSampler;

layout(std140) uniform MotionBlurConfig {
    float Strength;
};

layout(location = 0) in vec2 texCoord;

layout(location = 0) out vec4 fragColor;

void main() {
    vec3 now = texture(InSampler, texCoord).rgb;
    vec3 previous = texture(PrevSampler, texCoord).rgb;
    fragColor = vec4(mix(now, previous, Strength), 1.0);
}
"""),
    "_gl": ("motion_blur_gl", """#version 330

uniform sampler2D InSampler;
uniform sampler2D PrevSampler;

layout(std140) uniform MotionBlurConfig {
    float Strength;
};

in vec2 texCoord;

out vec4 fragColor;

void main() {
    vec3 now = texture(InSampler, texCoord).rgb;
    vec3 previous = texture(PrevSampler, texCoord).rgb;
    fragColor = vec4(mix(now, previous, Strength), 1.0);
}
"""),
}

BLIT = {"BlitConfig": [{"name": "ColorModulate", "type": "vec4", "value": [1.0, 1.0, 1.0, 1.0]}]}


def effect(shader, strength):
    def blit(source, target):
        return {
            "vertex_shader": "minecraft:core/screenquad",
            "fragment_shader": "minecraft:post/blit",
            "inputs": [{"sampler_name": "In", "target": source}],
            "output": target,
            "uniforms": BLIT,
        }

    return {
        "targets": {"swap": {}, "previous": {"persistent": True}},
        "passes": [
            {
                "vertex_shader": "minecraft:core/screenquad",
                "fragment_shader": f"arctic:post/{shader}",
                "inputs": [
                    {"sampler_name": "In", "target": "minecraft:main"},
                    {"sampler_name": "Prev", "target": "previous"},
                ],
                "output": "swap",
                "uniforms": {"MotionBlurConfig": [{"name": "Strength", "type": "float", "value": strength}]},
            },
            blit("swap", "previous"),
            blit("swap", "minecraft:main"),
        ],
    }


def main():
    (ASSETS / "shaders/post").mkdir(parents=True, exist_ok=True)
    (ASSETS / "post_effect").mkdir(parents=True, exist_ok=True)
    for suffix, (shader, source) in DIALECTS.items():
        (ASSETS / f"shaders/post/{shader}.fsh").write_text(source, encoding="utf-8", newline="\n")
        for level, strength in STRENGTHS.items():
            path = ASSETS / f"post_effect/motion_blur{suffix}_{level}.json"
            path.write_text(json.dumps(effect(shader, strength), indent=2) + "\n", encoding="utf-8", newline="\n")
    meta = {"pack": {"description": "Arctic Client effects (motion blur)", "min_format": 84, "max_format": 1000}}
    (ROOT / "pack.mcmeta").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8", newline="\n")
    print("motion blur effects written")


if __name__ == "__main__":
    main()
