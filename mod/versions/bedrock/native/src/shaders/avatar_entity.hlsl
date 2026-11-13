// Arctic entity material port. Inputs must come from one owning draw cohort.
// Shader model 5; compilation occurs only when a future runtime consumer asks.
cbuffer DrawInputs : register(b0) {
    column_major float4x4 ViewProjection;
    row_major float3x4 FogPosition;
    float4 ColorModulator;
    float4 VertexTint;
    float4 LightDirection0;
    float4 LightDirection1;
    float4 FogColor;
    float4 FogRanges;
    float4 OverlayColor;
    uint4 Features; // per-face lighting, cardinal lighting, emissive, overlay
    float4 Cutout;  // x: sampled texture alpha threshold
};
cbuffer LightmapInputs : register(b1) {
    float4 LightmapPixels[256]; // owned normalized 16x16 texture snapshot
};
Texture2D<float4> Artwork : register(t0);
SamplerState NearestRepeat : register(s0);

struct Vertex {
    float3 position : POSITION;
    float3 normal : NORMAL;
    float2 uv : TEXCOORD0;
    uint argb : COLOR0;
    uint light : TEXCOORD1;
};
struct Fragment {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    float4 front : COLOR0;
    float4 back : COLOR1;
    float4 lightmap : COLOR2;
    float4 overlay : COLOR3;
    float2 fogDistance : TEXCOORD1;
};

float4 read_lightmap(uint packed) {
    // Java UV2 is a signed-short pair, although its transport is one u32.
    int2 unpacked = int2(int(packed << 16u) >> 16, int(packed) >> 16);
    precise float2 uv = float2(unpacked) / 256.0;
    uv = clamp(uv + 0.03125, 0.03125, 0.96875);
    precise float2 at = uv * 16.0 - 0.5;
    int2 low = int2(floor(at));
    int2 high = min(low + 1, 15);
    float2 fraction = at - float2(low);
    precise float4 a = LightmapPixels[low.y * 16 + low.x];
    precise float4 b = LightmapPixels[low.y * 16 + high.x];
    precise float4 c = LightmapPixels[high.y * 16 + low.x];
    precise float4 d = LightmapPixels[high.y * 16 + high.x];
    precise float4 top = a + (b - a) * fraction.x;
    precise float4 bottom = c + (d - c) * fraction.x;
    return top + (bottom - top) * fraction.y;
}
float directional(float2 value) {
    precise float2 positive = max(value, 0.0);
    return min(1.0, (positive.x + positive.y) * 0.6 + 0.4);
}
Fragment vertex_main(Vertex input) {
    Fragment output;
    precise float4 clip = mul(ViewProjection, float4(input.position, 1.0));
    output.position = clip;
    precise float3 fogPosition = mul(FogPosition, float4(input.position, 1.0));
    output.fogDistance = float2(length(fogPosition), max(length(fogPosition.xz), abs(fogPosition.y)));
    precise float4 color = float4((input.argb >> 16u) & 255u, (input.argb >> 8u) & 255u,
        input.argb & 255u, input.argb >> 24u) / 255.0;
    // Normals were packed/unpacked as Java signed bytes by the producer.
    // No normalization here: that would change the vertex shader's lighting.
    precise float2 light = float2(dot(LightDirection0.xyz, input.normal), dot(LightDirection1.xyz, input.normal));
    float front = (Features.x != 0u || Features.y != 0u) ? directional(light) : 1.0;
    float back = Features.x != 0u ? directional(-light) : front;
    output.front = float4((color.rgb * front) * VertexTint.rgb, color.a * VertexTint.a);
    output.back = float4((color.rgb * back) * VertexTint.rgb, color.a * VertexTint.a);
    output.lightmap = Features.z != 0u ? float4(1,1,1,1) : read_lightmap(input.light);
    // entity.vsh fetches the selected texel per vertex and emits a smooth
    // overlayColor varying; it is not a fragment-stage uniform/flat varying.
    output.overlay = OverlayColor;
    output.uv = input.uv;
    return output;
}
float fog_fraction(float distance, float start, float end) {
    if (distance <= start) return 0.0;
    if (distance >= end) return 1.0;
    return (distance - start) / (end - start);
}
float4 pixel_main(Fragment input, bool front : SV_IsFrontFace) : SV_Target {
    precise float4 color = Artwork.Sample(NearestRepeat, input.uv);
    // This comparison precedes tint/actor fade, including zero-alpha draws.
    if (color.a < Cutout.x) discard;
    precise float4 vertex = (Features.x != 0u && !front) ? input.back : input.front;
    color *= vertex * ColorModulator;
    if (Features.w != 0u) color.rgb = input.overlay.rgb + (color.rgb - input.overlay.rgb) * input.overlay.a;
    if (Features.z == 0u) color *= input.lightmap;
    precise float amount = max(fog_fraction(input.fogDistance.x, FogRanges.x, FogRanges.y),
        fog_fraction(input.fogDistance.y, FogRanges.z, FogRanges.w));
    color.rgb = color.rgb + (FogColor.rgb - color.rgb) * (amount * FogColor.a);
    return color;
}
