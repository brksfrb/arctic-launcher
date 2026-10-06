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
    vec3 previous = texture(PrevSampler, texCoord).rgb;
    fragColor = vec4(mix(now, previous, Strength), 1.0);
}
