#version 330

// Minecraft's core/text.vsh (26.2) with its includes written out, except that
// the vertices come from Polonium's glyph pool. Each instance is one run of a
// name tag's quads (Item: first pool vertex, vertex count, tag); Slot is the
// vertex within the run, and slots past its end collapse to nothing. Pool
// vertices are two texels: x, y, z, -; u, v, color as two 16-bit halves.
// The tag's pose and light come from PoloniumInstances (pose: 3 rows of 4;
// then light u, v).

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
uniform samplerBuffer PoloniumGlyphs;

in int Slot;
in ivec3 Item;

#if !defined(IS_GUI) && !defined(IS_SEE_THROUGH)
uniform sampler2D Sampler2;
out float sphericalVertexDistance;
out float cylindricalVertexDistance;
#endif

out vec4 vertexColor;
out vec2 texCoord0;

void main() {
    if (Slot >= Item.y) {
        // Past this run's end: every such vertex lands on the same point outside the view.
        gl_Position = vec4(0.0, 0.0, 2.0, 1.0);
        vertexColor = vec4(0.0);
        texCoord0 = vec2(0.0);
#if !defined(IS_GUI) && !defined(IS_SEE_THROUGH)
        sphericalVertexDistance = 0.0;
        cylindricalVertexDistance = 0.0;
#endif
        return;
    }
    int glyph = (Item.x + Slot) * 2;
    vec4 first = texelFetch(PoloniumGlyphs, glyph);
    vec4 second = texelFetch(PoloniumGlyphs, glyph + 1);
    uint high = uint(second.z);
    uint low = uint(second.w);
    vec4 Color = vec4(float(high & 255u), float(low >> 8u), float(low & 255u), float(high >> 8u)) / 255.0;

    int tag = Item.z * 4;
    vec4 local = vec4(first.xyz, 1.0);
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
    texCoord0 = second.xy;
}
