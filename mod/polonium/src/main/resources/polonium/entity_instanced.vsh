#version 330

// Minecraft's core/entity.vsh (26.2) with its includes written out, except that
// vertices arrive in their model part's own space and are placed here, on the
// GPU: each part's matrix, and each entity's color, overlay and light, come
// from PoloniumInstances. One draw covers every entity sharing a model and
// texture (gl_InstanceID picks the entity).

#if defined(PER_FACE_LIGHTING) || !defined(NO_CARDINAL_LIGHTING)
#define MINECRAFT_LIGHT_POWER   (0.6)
#define MINECRAFT_AMBIENT_LIGHT (0.4)

layout(std140) uniform Lighting {
    vec3 Light0_Direction;
    vec3 Light1_Direction;
};

vec2 minecraft_compute_light(vec3 lightDir0, vec3 lightDir1, vec3 normal) {
    return vec2(dot(lightDir0, normal), dot(lightDir1, normal));
}

vec4 minecraft_mix_light_separate(vec2 light, vec4 color) {
    vec2 lightValue = max(vec2(0.0), light);
    float lightAccum = min(1.0, (lightValue.x + lightValue.y) * MINECRAFT_LIGHT_POWER + MINECRAFT_AMBIENT_LIGHT);
    return vec4(color.rgb * lightAccum, color.a);
}

vec4 minecraft_mix_light(vec3 lightDir0, vec3 lightDir1, vec3 normal, vec4 color) {
    vec2 light = minecraft_compute_light(lightDir0, lightDir1, normal);
    return minecraft_mix_light_separate(light, color);
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

// x: this draw's first texel in PoloniumInstances; y: texels per entity.
layout(std140) uniform PoloniumDraw {
    ivec4 PoloniumBase;
};

// Per entity: color, then (overlay u, v, light u, v), then where its texture
// is (u, v offset and scale: a cell of Polonium's skin atlas, or 0, 0, 1, 1),
// then three texels per model part: its pose (3 rows of 4). Normals use the pose's inverse
// transpose, which is what Minecraft's normal matrix is.
uniform samplerBuffer PoloniumInstances;

vec4 sample_lightmap(sampler2D lightMap, ivec2 uv) {
    return texture(lightMap, clamp((uv / 256.0) + 0.5 / 16.0, vec2(0.5 / 16.0), vec2(15.5 / 16.0)));
}

float fog_spherical_distance(vec3 pos) {
    return length(pos);
}

float fog_cylindrical_distance(vec3 pos) {
    float distXZ = length(pos.xz);
    float distY = abs(pos.y);
    return max(distXZ, distY);
}

in vec3 Position;
in vec2 UV0;
in ivec2 UV1;
in vec3 Normal;

#ifndef NO_OVERLAY
uniform sampler2D Sampler1;
#endif

#ifndef EMISSIVE
uniform sampler2D Sampler2;
#endif

out float sphericalVertexDistance;
out float cylindricalVertexDistance;

#ifdef PER_FACE_LIGHTING
out vec4 vertexPerFaceColorBack;
out vec4 vertexPerFaceColorFront;
#else
out vec4 vertexColor;
#endif

#ifndef EMISSIVE
out vec4 lightMapColor;
#endif

#ifndef NO_OVERLAY
out vec4 overlayColor;
#endif

out vec2 texCoord0;

void main() {
    int entity = PoloniumBase.x + gl_InstanceID * PoloniumBase.y;
    vec4 color = texelFetch(PoloniumInstances, entity);
    vec4 coords = texelFetch(PoloniumInstances, entity + 1);
    ivec2 overlayCoords = ivec2(coords.xy);
    ivec2 lightCoords = ivec2(coords.zw);

    vec4 placement = texelFetch(PoloniumInstances, entity + 2);
    int part = entity + 3 + UV1.x * 3;
    vec4 row0 = texelFetch(PoloniumInstances, part);
    vec4 row1 = texelFetch(PoloniumInstances, part + 1);
    vec4 row2 = texelFetch(PoloniumInstances, part + 2);
    vec4 local = vec4(Position, 1.0);
    vec3 position = vec3(dot(row0, local), dot(row1, local), dot(row2, local));
    mat3 linear = transpose(mat3(row0.xyz, row1.xyz, row2.xyz));
    vec3 normal = normalize(transpose(inverse(linear)) * Normal);

    gl_Position = ProjMat * ModelViewMat * vec4(position, 1.0);

    sphericalVertexDistance = fog_spherical_distance(position);
    cylindricalVertexDistance = fog_cylindrical_distance(position);

#ifdef PER_FACE_LIGHTING
    vec2 light = minecraft_compute_light(Light0_Direction, Light1_Direction, normal);
    vertexPerFaceColorBack = minecraft_mix_light_separate(-light, color);
    vertexPerFaceColorFront = minecraft_mix_light_separate(light, color);
#elif defined(NO_CARDINAL_LIGHTING)
    vertexColor = color;
#else
    vertexColor = minecraft_mix_light(Light0_Direction, Light1_Direction, normal, color);
#endif

#ifndef EMISSIVE
    lightMapColor = sample_lightmap(Sampler2, lightCoords);
#endif

#ifndef NO_OVERLAY
    overlayColor = texelFetch(Sampler1, overlayCoords, 0);
#endif

    texCoord0 = placement.xy + UV0 * placement.zw;

#ifdef APPLY_TEXTURE_MATRIX
    texCoord0 = (TextureMat * vec4(UV0, 0.0, 1.0)).xy;
#endif
}
