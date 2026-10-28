#version 330

uniform sampler2D InSampler;

in vec2 texCoord;

out vec4 fragColor;

layout(std140) uniform MotionBlurConfig {
    vec4 ToLast0;
    vec4 ToLast1;
    vec4 ToLast2;
    // tan(fov / 2) across, up, how much of a frame's movement is smeared, and the stretch from the middle.
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
    if (Params.z <= 0.0 && Params.w <= 0.0) {
        fragColor = vec4(texture(InSampler, texCoord).rgb, 1.0);
        return;
    }
    // Where this pixel was in the last frame, turned back by the camera's turn: the smear runs along it.
    vec2 move = (lastPosition(texCoord) - texCoord) * Params.z + (texCoord - vec2(0.5)) * Params.w;
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
