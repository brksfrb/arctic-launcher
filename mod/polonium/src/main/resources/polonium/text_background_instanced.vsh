#version 330

// Minecraft's core/text_background.vsh (26.2) with its includes written out, except that
// vertices arrive in their name tag's own space and are placed here, on the
// GPU: each vertex names its tag (Instance), whose pose and light come from
// PoloniumInstances (pose: 3 rows of 4; then light u, v). One draw covers
// every tag of a render type.

#if !defined(IS_GUI) && !defined(IS_SEE_THROUGH)
float fog_spherical_distance(vec3 pos) {
    return length(pos);
}

float fog_cylindrical_distance(vec3 pos) {
    float distXZ = length(pos.xz);
    float distY = abs(pos.y);
    return max(distXZ, distY);
}

vec4 sample_lightmap(sampler2D lightMap, ivec2 uv) {
    return texture(lightMap, clamp((uv / 256.0) + 0.5 / 16.0, vec2(0.5 / 16.0), vec2(15.5 / 16.0)));
}
#endif

layout(std140) uniform DynamicTransforms {
    mat4 ModelViewMat;
    vec4 ColorModulator;
    vec3 ModelOffset;
    mat4 TextureMat;
};

layout(std140) uniform Projection {
    mat4 ProjMat;
};

uniform samplerBuffer PoloniumInstances;

in vec3 Position;
in vec4 Color;

in int Instance;

#if !defined(IS_GUI) && !defined(IS_SEE_THROUGH)
uniform sampler2D Sampler2;
out float sphericalVertexDistance;
out float cylindricalVertexDistance;
#endif

out vec4 vertexColor;


void main() {
    int tag = Instance * 4;
    vec4 local = vec4(Position, 1.0);
    vec3 position = vec3(
        dot(texelFetch(PoloniumInstances, tag), local),
        dot(texelFetch(PoloniumInstances, tag + 1), local),
        dot(texelFetch(PoloniumInstances, tag + 2), local));
    gl_Position = ProjMat * ModelViewMat * vec4(position, 1.0);

#if !defined(IS_GUI) && !defined(IS_SEE_THROUGH)
    sphericalVertexDistance = fog_spherical_distance(position);
    cylindricalVertexDistance = fog_cylindrical_distance(position);
    ivec2 lightCoords = ivec2(texelFetch(PoloniumInstances, tag + 3).xy);
    vertexColor = Color * sample_lightmap(Sampler2, lightCoords);
#else
    vertexColor = Color;
#endif
}
