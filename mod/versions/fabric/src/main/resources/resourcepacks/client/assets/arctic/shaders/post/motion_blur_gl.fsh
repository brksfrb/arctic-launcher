#version 330

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
