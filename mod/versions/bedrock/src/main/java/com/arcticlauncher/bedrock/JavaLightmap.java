package com.arcticlauncher.bedrock;

/** Explicit-input source port of Java 26.2 lightmap.fsh. No world, wall clock,
 * random flicker source or Bedrock named uniform is inferred by this class. */
final class JavaLightmap {
    static final class Inputs {
        int schema;
        Float skyFactor,blockFactor,nightVisionFactor,darknessScale,bossOverlayWorldDarkeningFactor,brightnessFactor;
        float[] blockLightTint,skyLightColor,ambientColor,nightVisionColor;
        /** Explicit source-backend RGBA8 conversion policy. Neither policy
         * establishes equivalence to an unobserved GPU implementation. */
        String unormRounding;
    }
    private JavaLightmap() {}

    /** Owned row-major RGBA values: X is block level, Y is sky level. The
     * values have passed through the selected RGBA8_UNORM conversion before
     * they are sampled by the entity shader's bilinear lightmap stage. */
    static float[] generate(Inputs input) {
        if(input==null||input.schema!=1)throw new IllegalArgumentException("unsupported Java lightmap inputs");
        float skyFactor=required(input.skyFactor),blockFactor=required(input.blockFactor);
        float nightVision=required(input.nightVisionFactor),darkness=required(input.darknessScale);
        float boss=required(input.bossOverlayWorldDarkeningFactor),brightnessMix=required(input.brightnessFactor);
        float[] blockTint=vector(input.blockLightTint),skyColor=vector(input.skyLightColor);
        float[] ambient=vector(input.ambientColor),nightColor=vector(input.nightVisionColor);
        String rounding=input.unormRounding;
        if(!"nearest-even".equals(rounding)&&!"nearest-up".equals(rounding))
            throw new IllegalArgumentException("Java lightmap needs an explicit RGBA8 conversion policy");
        float[] output=new float[16*16*4],color=new float[3];
        for(int sky=0;sky<16;sky++)for(int block=0;block<16;block++) {
            // A full-screen lightmap texel's centre gives floor(UV*16)=index.
            float blockLevel=block/15f,skyLevel=sky/15f;
            float blockBrightness=brightness(blockLevel)*blockFactor;
            float skyBrightness=brightness(skyLevel)*skyFactor;
            float parabola=2f*blockLevel-1f;
            float whitening=.9f*(parabola*parabola);
            for(int channel=0;channel<3;channel++) {
                float value=Math.max(ambient[channel],nightColor[channel]*nightVision);
                value+=skyColor[channel]*skyBrightness;
                value+=mix(blockTint[channel],1f,whitening)*blockBrightness;
                value=mix(value,value*(channel==0?.7f:.6f),boss);
                color[channel]=clamp(value-darkness);
            }
            float maximum=Math.max(Math.max(color[0],color[1]),color[2]);
            // The literal shader divides by its maximum component. At zero,
            // notGamma produces 0/0; backend treatment of that NaN is not a
            // portable RGBA8 rule. Require captured pixels for such a state.
            if(!Float.isFinite(maximum)||maximum==0)
                throw new IllegalArgumentException("Java lightmap shader has an unresolved zero/nonfinite gamma maximum");
            float inverted=1f-maximum;
            float scaled=1f-((inverted*inverted)*inverted)*inverted;
            float gammaScale=scaled/maximum;
            int at=(sky*16+block)*4;
            for(int channel=0;channel<3;channel++) {
                float value=mix(color[channel],color[channel]*gammaScale,brightnessMix);
                if(!Float.isFinite(value))throw new IllegalArgumentException("nonfinite generated Java lightmap channel");
                double scaledByte=(double)clamp(value)*255d;
                int quantized=(int)("nearest-even".equals(rounding)?Math.rint(scaledByte):Math.floor(scaledByte+.5d));
                output[at+channel]=quantized/255f;
            }
            output[at+3]=1f;
        }
        return output;
    }
    private static float required(Float value) {
        if(value==null||!Float.isFinite(value))throw new IllegalArgumentException("missing/nonfinite Java lightmap factor");
        return value;
    }
    private static float[] vector(float[] values) {
        if(values==null||values.length!=3)throw new IllegalArgumentException("missing Java lightmap color");
        float[] result=values.clone();
        for(float value:result)if(!Float.isFinite(value))throw new IllegalArgumentException("nonfinite Java lightmap color");
        return result;
    }
    private static float brightness(float level){return level/(4f-3f*level);}
    private static float mix(float a,float b,float factor){return a*(1f-factor)+b*factor;}
    private static float clamp(float value){return Math.max(0f,Math.min(1f,value));}
}
