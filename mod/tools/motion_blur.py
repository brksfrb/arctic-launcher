"""Write the motion blur post effects into the Arctic Client's built-in pack (26.1 and later).

Usage: python mod/tools/motion_blur.py
Writes mod/versions/fabric/src/main/resources/resourcepacks/client/assets/arctic/...

Two families:

"smear" (26.1 and later): each pixel is smeared along the way the picture moved since the last frame,
worked out from how the camera turned (mod/core MotionBlurView). The numbers change every frame: the
mod writes them into the effect's uniform buffer (MotionBlurPostMixin / MotionBlurUniforms), the JSON
only declares the layout.

"blend" (VulkanMod on 1.21.6-1.21.11, where the smear can't be fed): each frame is blended with the last
blended frame, kept in a persistent target:
out = mix(now, previous, strength), previous = out. A previous frame that wasn't kept
(cleared to transparent black) isn't blended in. One post effect per strength
(post-effect uniforms are fixed in the JSON) and per shader dialect: 26.3's
shaders are Vulkan-style GLSL, 26.1 and 26.2 plain GLSL 330.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "versions/fabric/src/main/resources/resourcepacks/client"
ASSETS = ROOT / "assets/arctic"

# Strength 1 (Low) to 4 (Max): how much of the previous frame stays where the picture is still.
STRENGTHS = {1: 0.4, 2: 0.55, 3: 0.68, 4: 0.78}

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
    // A kept frame is opaque; a previous-frame buffer some drivers lose (or wipe) reads as
    // transparent black: blend only with what was really kept, or the picture darkens.
    vec4 previous = texture(PrevSampler, texCoord);
    // Only where the picture changed completely (a flick, a teleport) the old frame is let go, so a
    // ghost of the old view never stays; ordinary movement keeps the full blur.
    float change = length(now - previous.rgb);
    float keep = Strength * previous.a * (1.0 - smoothstep(0.3, 0.8, change));
    fragColor = vec4(mix(now, previous.rgb, keep), 1.0);
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
    // A kept frame is opaque; a previous-frame buffer some drivers lose (or wipe) reads as
    // transparent black: blend only with what was really kept, or the picture darkens.
    vec4 previous = texture(PrevSampler, texCoord);
    // Only where the picture changed completely (a flick, a teleport) the old frame is let go, so a
    // ghost of the old view never stays; ordinary movement keeps the full blur.
    float change = length(now - previous.rgb);
    float keep = Strength * previous.a * (1.0 - smoothstep(0.3, 0.8, change));
    fragColor = vec4(mix(now, previous.rgb, keep), 1.0);
}
"""),
}

SMEAR_BODY = """
layout(std140) uniform MotionBlurConfig {
    vec4 ToLast0;
    vec4 ToLast1;
    vec4 ToLast2;
    // tan(fov / 2) across, up, and how much of a frame's movement is smeared.
    vec4 Params;
};

const int SAMPLES = 16;
const float LONGEST = 0.25;

vec2 lastPosition(vec2 at) {
    vec2 ndc = at * 2.0 - 1.0;
    vec3 p = mat3(ToLast0.xyz, ToLast1.xyz, ToLast2.xyz) * vec3(ndc.x * Params.x, ndc.y * Params.y, -1.0);
    return vec2(p.x / -p.z / Params.x, p.y / -p.z / Params.y) * 0.5 + 0.5;
}

void main() {
    if (Params.z <= 0.0) {
        fragColor = vec4(texture(InSampler, texCoord).rgb, 1.0);
        return;
    }
    // Where this pixel was in the last frame, turned back by the camera's turn: the smear runs along it.
    vec2 move = (lastPosition(texCoord) - texCoord) * Params.z;
    float moved = length(move);
    if (moved > LONGEST) {
        move *= LONGEST / moved;
    }
    // A little noise per pixel moves the samples, so they don't show as steps.
    float noise = fract(52.9829189 * fract(dot(gl_FragCoord.xy, vec2(0.06711056, 0.00583715))));
    vec3 sum = vec3(0.0);
    for (int i = 0; i < SAMPLES; i++) {
        float t = (float(i) + noise) / float(SAMPLES) - 0.5;
        sum += texture(InSampler, clamp(texCoord + move * t, 0.0, 1.0)).rgb;
    }
    fragColor = vec4(sum / float(SAMPLES), 1.0);
}
"""

SMEAR_DIALECTS = {
    "": ("motion_blur_smear", """#version 330
#extension GL_ARB_separate_shader_objects : require

uniform sampler2D InSampler;

layout(location = 0) in vec2 texCoord;

layout(location = 0) out vec4 fragColor;
""" + SMEAR_BODY),
    "_gl": ("motion_blur_smear_gl", """#version 330

uniform sampler2D InSampler;

in vec2 texCoord;

out vec4 fragColor;
""" + SMEAR_BODY),
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
        # Cleared to transparent (not the default opaque black): a frame the game didn't keep
        # reads as see-through, and the shader leaves it out.
        "targets": {"swap": {}, "previous": {"persistent": True, "clear_color": 0}},
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
            # Back to the screen from "previous" (the same picture): the frame graph drops passes
            # whose output nothing reads that frame, so read it, or it's never kept (and every
            # frame is blended with black: a dark game).
            blit("previous", "minecraft:main"),
        ],
    }


def smear_effect(shader):
    zero4 = [0.0, 0.0, 0.0, 0.0]
    return {
        "targets": {"swap": {}},
        "passes": [
            {
                "vertex_shader": "minecraft:core/screenquad",
                "fragment_shader": f"arctic:post/{shader}",
                "inputs": [{"sampler_name": "In", "target": "minecraft:main", "bilinear": True}],
                "output": "swap",
                # Placeholders: the mod writes the real numbers every frame.
                "uniforms": {"MotionBlurConfig": [
                    {"name": "ToLast0", "type": "vec4", "value": [1.0, 0.0, 0.0, 0.0]},
                    {"name": "ToLast1", "type": "vec4", "value": [0.0, 1.0, 0.0, 0.0]},
                    {"name": "ToLast2", "type": "vec4", "value": [0.0, 0.0, 1.0, 0.0]},
                    {"name": "Params", "type": "vec4", "value": zero4},
                ]},
            },
            {
                "vertex_shader": "minecraft:core/screenquad",
                "fragment_shader": "minecraft:post/blit",
                "inputs": [{"sampler_name": "In", "target": "swap"}],
                "output": "minecraft:main",
                "uniforms": BLIT,
            },
        ],
    }


def main():
    (ASSETS / "shaders/post").mkdir(parents=True, exist_ok=True)
    (ASSETS / "post_effect").mkdir(parents=True, exist_ok=True)
    for old in (ASSETS / "post_effect").glob("motion_blur*.json"):
        old.unlink()
    for suffix, (shader, source) in SMEAR_DIALECTS.items():
        (ASSETS / f"shaders/post/{shader}.fsh").write_text(source, encoding="utf-8", newline="\n")
        path = ASSETS / f"post_effect/motion_blur_smear{suffix}.json"
        path.write_text(json.dumps(smear_effect(shader), indent=2) + "\n", encoding="utf-8", newline="\n")
    for suffix, (shader, source) in DIALECTS.items():
        (ASSETS / f"shaders/post/{shader}.fsh").write_text(source, encoding="utf-8", newline="\n")
        for level, strength in STRENGTHS.items():
            path = ASSETS / f"post_effect/motion_blur{suffix}_{level}.json"
            path.write_text(json.dumps(effect(shader, strength), indent=2) + "\n", encoding="utf-8", newline="\n")
    # 1.21.6 (format 63) on: before 1.21.9 the game reads pack_format/supported_formats, after it
    # min_format/max_format.
    meta = {"pack": {
        "description": "Arctic Client effects (motion blur)",
        "pack_format": 63,
        "supported_formats": {"min_inclusive": 63, "max_inclusive": 1000},
        "min_format": 63,
        "max_format": 1000,
    }}
    (ROOT / "pack.mcmeta").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8", newline="\n")
    print("motion blur effects written")


if __name__ == "__main__":
    main()
