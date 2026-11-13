package com.arcticlauncher.bedrock;

import java.io.*;
import java.nio.ByteBuffer;
import java.util.Arrays;

/** Explicit-input Java 26.2 entity.vsh/entity.fsh stages. Input arrays are
 * copied before emission; no Bedrock shader constant is reinterpreted here. */
final class JavaEntityShader {
    static final int WIRE_FLOATS=1058;
    /** JSON adapter contract, scoped to the containing world observation.
     * Normals and positions must share the declared emitted coordinate space.
     * The producer must supply all fields, including the selected NO_OVERLAY
     * texel. These fields do not establish a native producer receipt. */
    static final class Inputs {
        int schema;
        float[] lightmap,colorModulator,lightDirections,fogPosition,overlayColor,fogColor,fogRanges;
        JavaLightmap.Inputs lightmapState;
    }
    private final float[] lightmap,modulator,directions,fogPosition,overlay,fogColor,fogRanges;

    JavaEntityShader(Inputs input) {
        if(input==null||input.schema!=1)throw new IllegalArgumentException("unsupported entity shader inputs");
        if((input.lightmap==null)==(input.lightmapState==null))
            throw new IllegalArgumentException("entity shader needs exactly one lightmap pixel/state source");
        lightmap=input.lightmap!=null?copy(input.lightmap,1024,true):JavaLightmap.generate(input.lightmapState);
        modulator=copy(input.colorModulator,4,false);directions=copy(input.lightDirections,6,false);
        fogPosition=copy(input.fogPosition,12,false);overlay=copy(input.overlayColor,4,true);
        fogColor=copy(input.fogColor,4,false);fogRanges=copy(input.fogRanges,4,false);
        if(fogRanges[1]<fogRanges[0]||fogRanges[3]<fogRanges[2])
            throw new IllegalArgumentException("reversed entity shader fog ranges");
    }
    private static float[] copy(float[] source,int length,boolean normalized) {
        if(source==null||source.length!=length)throw new IllegalArgumentException("missing entity shader input");
        float[] result=source.clone();
        for(float value:result)if(!Float.isFinite(value)||normalized&&(value<0||value>1))
            throw new IllegalArgumentException("invalid entity shader input");
        return result;
    }
    boolean same(JavaEntityShader other) {
        return other!=null&&Arrays.equals(lightmap,other.lightmap)&&Arrays.equals(modulator,other.modulator)
            &&Arrays.equals(directions,other.directions)&&Arrays.equals(fogPosition,other.fogPosition)
            &&Arrays.equals(overlay,other.overlay)&&Arrays.equals(fogColor,other.fogColor)&&Arrays.equals(fogRanges,other.fogRanges);
    }
    /** entity.vsh computes fog from Position before ModelViewMat. For emitted
     * camera-relative world coordinates the identity transform remains valid
     * under a replay camera edit; other transforms need their source context. */
    boolean cameraRelativeWorldFog() {
        return Arrays.equals(fogPosition,new float[]{1,0,0,0,0,1,0,0,0,0,1,0});
    }
    /** Selected common NO_OVERLAY texel for the vertex-stage varying. Skin
     * draws supply their independently copied selected texel instead. */
    float[] overlayColor() {return overlay.clone();}
    void write(DataOutput output) throws IOException {
        for(float[] values:new float[][]{lightmap,modulator,directions,fogPosition,overlay,fogColor,fogRanges})
            for(float value:values)output.writeFloat(value);
    }
    /** Same scalar order as the archive, with caller-selected packet endian. */
    void write(ByteBuffer output) {
        for(float[] values:new float[][]{lightmap,modulator,directions,fogPosition,overlay,fogColor,fogRanges})
            for(float value:values)output.putFloat(value);
    }
    static JavaEntityShader read(DataInput input) throws IOException {
        Inputs values=new Inputs();values.schema=1;
        values.lightmap=read(input,1024);values.colorModulator=read(input,4);values.lightDirections=read(input,6);
        values.fogPosition=read(input,12);values.overlayColor=read(input,4);values.fogColor=read(input,4);values.fogRanges=read(input,4);
        try{return new JavaEntityShader(values);}catch(IllegalArgumentException invalid){throw new IOException("invalid recorded entity material",invalid);}
    }
    private static float[] read(DataInput input,int count) throws IOException {
        float[] result=new float[count];for(int i=0;i<count;i++)result[i]=input.readFloat();return result;
    }
    float shade(float nx,float ny,float nz,boolean front) {
        float a=directions[0]*nx+directions[1]*ny+directions[2]*nz;
        float b=directions[3]*nx+directions[4]*ny+directions[5]*nz;
        if(!front){a=-a;b=-b;}
        return Math.min(1f,(Math.max(a,0)+Math.max(b,0))*.6f+.4f);
    }
    /** sample_lightmap.glsl: signed short UV2, normalized clamp and bilinear
     * sampling at texel centres. Output is caller-owned four-float scratch. */
    void sampleLightmap(int packed,float[] output) {
        float u=Math.max(.03125f,Math.min(.96875f,(short)packed/256f+.03125f));
        float v=Math.max(.03125f,Math.min(.96875f,(short)(packed>>>16)/256f+.03125f));
        float x=u*16-.5f,y=v*16-.5f;
        int x0=(int)Math.floor(x),y0=(int)Math.floor(y),x1=Math.min(15,x0+1),y1=Math.min(15,y0+1);
        float ax=x-x0,ay=y-y0;
        int a=(y0*16+x0)*4,b=(y0*16+x1)*4,c=(y1*16+x0)*4,d=(y1*16+x1)*4;
        for(int channel=0;channel<4;channel++) {
            float top=lightmap[a+channel]+(lightmap[b+channel]-lightmap[a+channel])*ax;
            float bottom=lightmap[c+channel]+(lightmap[d+channel]-lightmap[c+channel])*ax;
            output[channel]=top+(bottom-top)*ay;
        }
    }
    /** Row-major affine transform from emitted positions to Java fog space. */
    void fogDistance(float x,float y,float z,float[] output) {
        float fx=fogPosition[0]*x+fogPosition[1]*y+fogPosition[2]*z+fogPosition[3];
        float fy=fogPosition[4]*x+fogPosition[5]*y+fogPosition[6]*z+fogPosition[7];
        float fz=fogPosition[8]*x+fogPosition[9]*y+fogPosition[10]*z+fogPosition[11];
        output[0]=(float)Math.sqrt(fx*fx+fy*fy+fz*fz);
        output[1]=Math.max((float)Math.sqrt(fx*fx+fz*fz),Math.abs(fy));
    }
    /** rgba initially contains the interpolated face vertex color. The caller
     * performs the sampled-alpha cutout before this method and quantizes once
     * after these stages. Overlay/fog leave alpha intact; lightmap does not. */
    void fragment(int sample,float[] rgba,boolean useOverlay,boolean emissive,float[] light,
            float spherical,float cylindrical) {
        fragment(sample,rgba,useOverlay,emissive,light,spherical,cylindrical,null);
    }
    void fragment(int sample,float[] rgba,boolean useOverlay,boolean emissive,float[] light,
            float spherical,float cylindrical,float[] interpolatedOverlay) {
        // Direct explicit fragment callers may select the common constant;
        // Raster3D supplies the perspective-interpolated vertex-stage result.
        float[] overlayInput=interpolatedOverlay==null?overlay:interpolatedOverlay;
        float fog=Math.max(fraction(spherical,fogRanges[0],fogRanges[1]),
            fraction(cylindrical,fogRanges[2],fogRanges[3]))*fogColor[3];
        for(int channel=0;channel<4;channel++) {
            int shift=channel==3?24:16-channel*8;
            float value=((sample>>>shift)&255)/255f*(rgba[channel]*modulator[channel]);
            if(channel<3&&useOverlay)value=overlayInput[channel]+(value-overlayInput[channel])*overlayInput[3];
            if(!emissive)value*=light[channel];
            if(channel<3)value=value+(fogColor[channel]-value)*fog;
            rgba[channel]=value;
        }
    }
    private static float fraction(float distance,float start,float end) {
        if(distance<=start)return 0;
        if(distance>=end)return 1;
        return (distance-start)/(end-start);
    }
}
