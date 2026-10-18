#version 330
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
    // Where the picture changed a lot (you turned), the old frame is let go at once, so nothing
    // stays behind; where it barely changed, it smooths. A fixed blend left a ghost on every flick.
    float change = length(now - previous.rgb);
    float keep = Strength * previous.a * (1.0 - smoothstep(0.04, 0.35, change));
    fragColor = vec4(mix(now, previous.rgb, keep), 1.0);
}
