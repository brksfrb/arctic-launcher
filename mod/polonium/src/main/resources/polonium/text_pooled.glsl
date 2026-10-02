// Polonium: name tag text from Polonium's glyph pool. Each instance is
// one run of a name tag's quads (Item: first pool vertex, vertex count, tag);
// Slot is the vertex within the run, and slots past its end collapse to
// nothing. Pool vertices are two texels: x, y, z, -; u, v, color as two 16-bit
// halves. The tag's pose and light come from PoloniumInstances (pose: 3 rows
// of 4; then light u, v). The game's own shader (core/text) does the rest.

uniform samplerBuffer PoloniumInstances;
uniform samplerBuffer PoloniumGlyphs;

in int Slot;
in ivec3 Item;

void polonium_main() {
    if (Slot >= Item.y) {
        // Past this run's end: every such vertex lands on the same point outside the view.
        minecraft_Position = vec3(0.0);
        minecraft_Color = vec4(0.0);
        minecraft_UV0 = vec2(0.0);
        minecraft_UV2 = ivec2(0);
        minecraft_main();
        gl_Position = vec4(0.0, 0.0, 2.0, 1.0);
        return;
    }
    int glyph = (Item.x + Slot) * 2;
    vec4 first = texelFetch(PoloniumGlyphs, glyph);
    vec4 second = texelFetch(PoloniumGlyphs, glyph + 1);
    uint high = uint(second.z);
    uint low = uint(second.w);
    int tag = Item.z * 4;
    vec4 local = vec4(first.xyz, 1.0);

    minecraft_Position = vec3(
        dot(texelFetch(PoloniumInstances, tag), local),
        dot(texelFetch(PoloniumInstances, tag + 1), local),
        dot(texelFetch(PoloniumInstances, tag + 2), local));
    minecraft_Color = vec4(float(high & 255u), float(low >> 8u), float(low & 255u), float(high >> 8u)) / 255.0;
    minecraft_UV0 = second.xy;
    minecraft_UV2 = ivec2(texelFetch(PoloniumInstances, tag + 3).xy);
    minecraft_main();
}
