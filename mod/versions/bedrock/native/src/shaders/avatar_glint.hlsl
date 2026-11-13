// Java 26.2 armor glint. Every uniform is an explicit owning input.
cbuffer GlintInputs : register(b0) {
    column_major float4x4 ViewProjection;
    column_major float4x4 TextureMatrix;
    row_major float3x4 FogPosition;
    float4 ColorModulator;
    float4 FogRanges;
    float4 GlintAlpha; // x: observed strength, not sampled alpha
};
Texture2D<float4> Artwork : register(t0);
SamplerState LinearRepeat : register(s0);
struct Vertex {float3 position : POSITION;float2 uv : TEXCOORD0;};
struct Fragment {float4 position : SV_Position;float2 uv : TEXCOORD0;float2 fogDistance : TEXCOORD1;};
Fragment vertex_main(Vertex input) {
    Fragment output;
    precise float4 clip = mul(ViewProjection, float4(input.position, 1.0));
    output.position = clip;
    precise float3 fogPosition = mul(FogPosition, float4(input.position, 1.0));
    output.fogDistance = float2(length(fogPosition), max(length(fogPosition.xz), abs(fogPosition.y)));
    precise float4 transformed = mul(TextureMatrix, float4(input.uv, 0.0, 1.0));
    output.uv = transformed.xy;
    return output;
}
float fog_fraction(float distance, float start, float end) {
    if (distance <= start) return 0.0;
    if (distance >= end) return 1.0;
    return (distance - start) / (end - start);
}
float4 pixel_main(Fragment input) : SV_Target {
    precise float4 color = Artwork.Sample(LinearRepeat, input.uv) * ColorModulator;
    if (color.a < 0.1) discard;
    precise float fog = max(fog_fraction(input.fogDistance.x, FogRanges.x, FogRanges.y),
        fog_fraction(input.fogDistance.y, FogRanges.z, FogRanges.w));
    precise float fade = (1.0 - fog) * GlintAlpha.x;
    return float4(color.rgb * fade, color.a);
}
