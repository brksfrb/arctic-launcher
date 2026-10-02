// Polonium: entity models drawn instanced. Vertices arrive in their model
// part's own space and are placed here, on the GPU: each part's matrix, and
// each entity's color, overlay and light, come from PoloniumInstances. One
// draw covers every entity sharing a model and texture (gl_InstanceID picks
// the entity). The game's own entity shader (core/entity) does the rest.

// x: this draw's first texel in PoloniumInstances; y: texels per entity.
layout(std140) uniform PoloniumDraw {
    ivec4 PoloniumBase;
};

// Per entity: color, then (overlay u, v, light u, v), then where its texture
// is (u, v offset and scale: a cell of Polonium's skin atlas, or 0, 0, 1, 1),
// then where its parts' poses are (x: -1 right after this header; else the
// texel they start at, another entity's: armor posed exactly like the body it's
// on uses the body's), then three texels per model part: its pose (3 rows of
// 4). Normals use the pose's inverse transpose, which is what Minecraft's
// normal matrix is.
uniform samplerBuffer PoloniumInstances;

in vec3 Position;
in vec2 UV0;
in ivec2 UV1; // x: the vertex's model part
in vec3 Normal;

void polonium_main() {
    int entity = PoloniumBase.x + gl_InstanceID * PoloniumBase.y;
    vec4 color = texelFetch(PoloniumInstances, entity);
    vec4 coords = texelFetch(PoloniumInstances, entity + 1);
    vec4 placement = texelFetch(PoloniumInstances, entity + 2);
    float partsAt = texelFetch(PoloniumInstances, entity + 3).x;
    int part = (partsAt < 0.0 ? entity + 4 : int(partsAt)) + UV1.x * 3;
    vec4 row0 = texelFetch(PoloniumInstances, part);
    vec4 row1 = texelFetch(PoloniumInstances, part + 1);
    vec4 row2 = texelFetch(PoloniumInstances, part + 2);
    vec4 local = vec4(Position, 1.0);
    mat3 linear = transpose(mat3(row0.xyz, row1.xyz, row2.xyz));

    minecraft_Position = vec3(dot(row0, local), dot(row1, local), dot(row2, local));
    minecraft_Normal = normalize(transpose(inverse(linear)) * Normal);
    minecraft_Color = color;
    minecraft_UV0 = placement.xy + UV0 * placement.zw;
    minecraft_UV1 = ivec2(coords.xy);
    minecraft_UV2 = ivec2(coords.zw);
    minecraft_main();
}
