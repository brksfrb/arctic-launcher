#version 330

// Minecraft's core/item.vsh (26.2) with its includes written out, except that
// vertices arrive in the item's own space and are placed here, on the GPU,
// from each entity's pose in PoloniumInstances, which also holds its light,
// overlay and tint colors. Each vertex carries its quad's tint layer and
// light emission (UV1), applied as VertexConsumer.putBakedQuad does.

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

// Per entity: (overlay u, v, light u, v), the pose (3 rows of 4), then four
// tint layer colors.
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
in ivec2 UV1; // x: tint layer (-1 for none), y: light emission
in vec3 Normal;

uniform sampler2D Sampler1;
uniform sampler2D Sampler2;

out float sphericalVertexDistance;
out float cylindricalVertexDistance;
out vec4 vertexColor;
out vec4 lightMapColor;
out vec4 overlayColor;

out vec2 texCoord0;

void main() {
    int entity = PoloniumBase.x + gl_InstanceID * PoloniumBase.y;
    vec4 coords = texelFetch(PoloniumInstances, entity);
    vec4 row0 = texelFetch(PoloniumInstances, entity + 1);
    vec4 row1 = texelFetch(PoloniumInstances, entity + 2);
    vec4 row2 = texelFetch(PoloniumInstances, entity + 3);
    vec4 color = UV1.x >= 0 ? texelFetch(PoloniumInstances, entity + 4 + UV1.x) : vec4(1.0);

    // LightCoordsUtil.lightCoordsWithEmission: each channel at least the emission.
    ivec2 lightCoords = ivec2(coords.zw);
    if (UV1.y > 0) {
        lightCoords = max(lightCoords >> 4, ivec2(UV1.y)) << 4;
    }
    ivec2 overlayCoords = ivec2(coords.xy);

    vec4 local = vec4(Position, 1.0);
    vec3 position = vec3(dot(row0, local), dot(row1, local), dot(row2, local));
    mat3 linear = transpose(mat3(row0.xyz, row1.xyz, row2.xyz));
    vec3 normal = normalize(transpose(inverse(linear)) * Normal);

    gl_Position = ProjMat * ModelViewMat * vec4(position, 1.0);

    sphericalVertexDistance = fog_spherical_distance(position);
    cylindricalVertexDistance = fog_cylindrical_distance(position);

    vertexColor = minecraft_mix_light(Light0_Direction, Light1_Direction, normal, color);
    lightMapColor = sample_lightmap(Sampler2, lightCoords);
    overlayColor = texelFetch(Sampler1, overlayCoords, 0);

    texCoord0 = UV0;
}
