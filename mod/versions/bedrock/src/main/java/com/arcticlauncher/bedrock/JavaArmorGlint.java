package com.arcticlauncher.bedrock;

import java.awt.image.BufferedImage;

/** Copied Java 26.2 GLINT inputs. No wall-clock access or implicit options:
 * the caller supplies an explicit sample or observed clock/options/fog.
 * This is a software shader port, not proof of native/GPU equivalence. */
final class JavaArmorGlint {
    private static final float ROTATION=.1745329201221466f,SCALE=.1599999964237213f;
    private final float m00,m01,m10,m11,tx,ty;
    private final float strength,environmentStart,environmentEnd,distanceStart,distanceEnd;
    private final float[] modulator;
    private final float[] textureMatrix,fogPosition;

    /** Consume the same complete selected observation as native preparation.
     * Keep all matrix coefficients; do not reconstruct a clock or decompose
     * the observed matrix into an approximate rotation/scale/translation. */
    JavaArmorGlint(GlintSample selected) {
        if(selected==null||!selected.selected)throw new IllegalArgumentException("glint needs an active observation");
        float[] values=selected.values();
        textureMatrix=java.util.Arrays.copyOfRange(values,0,16);
        fogPosition=java.util.Arrays.copyOfRange(values,16,28);
        modulator=java.util.Arrays.copyOfRange(values,28,32);
        environmentStart=values[32];environmentEnd=values[33];
        distanceStart=values[34];distanceEnd=values[35];strength=values[36];
        m00=m01=m10=m11=tx=ty=0;
    }

    JavaArmorGlint(long javaUtilMillis,double speed,float strength,float[] modulator,
                   float environmentStart,float environmentEnd,float distanceStart,float distanceEnd) {
        if(!Double.isFinite(speed)||speed<0||speed>1||!Float.isFinite(strength)||strength<0||strength>1)
            throw new IllegalArgumentException("invalid glint options");
        WorldFrame.color(modulator);
        if(modulator==null)throw new IllegalArgumentException("glint needs an observed ColorModulator");
        for(float value:new float[]{environmentStart,environmentEnd,distanceStart,distanceEnd})
            if(!Float.isFinite(value))throw new IllegalArgumentException("invalid glint fog");
        if(environmentEnd<environmentStart||distanceEnd<distanceStart)throw new IllegalArgumentException("reversed glint fog");
        this.strength=strength;this.modulator=modulator.clone();
        textureMatrix=null;fogPosition=null;
        this.environmentStart=environmentStart;this.environmentEnd=environmentEnd;
        this.distanceStart=distanceStart;this.distanceEnd=distanceEnd;
        // TextureTransform.setupGlintTexturing. Preserve the double multiply,
        // signed long remainder, float division and narrowing conversion.
        long phase=(long)((double)javaUtilMillis*speed*8d);
        tx=-(float)(phase%110000L)/110000f;ty=(float)(phase%30000L)/30000f;
        float sine=(float)Math.sin(ROTATION);
        // JOML's default cosFromSin: this constant angle is in quadrant one.
        float cosine=(float)Math.sqrt(1-sine*sine);
        m00=cosine*SCALE;m01=sine*SCALE;m10=-sine*SCALE;m11=cosine*SCALE;
    }

    float u(float u,float v){return textureMatrix==null?(m00*u+m10*v)+tx
        :((textureMatrix[0]*u+textureMatrix[4]*v)+textureMatrix[8]*0f)+textureMatrix[12];}
    float v(float u,float v){return textureMatrix==null?(m01*u+m11*v)+ty
        :((textureMatrix[1]*u+textureMatrix[5]*v)+textureMatrix[9]*0f)+textureMatrix[13];}

    /** Row-major affine transform of the emitted position, before clipping.
     * The legacy explicit-clock helper retains its caller's identity-space
     * fog convention. Observed samples always use their supplied transform. */
    void fogDistance(float x,float y,float z,float[] output) {
        if(fogPosition!=null) {
            float a=((fogPosition[0]*x+fogPosition[1]*y)+fogPosition[2]*z)+fogPosition[3];
            float b=((fogPosition[4]*x+fogPosition[5]*y)+fogPosition[6]*z)+fogPosition[7];
            float c=((fogPosition[8]*x+fogPosition[9]*y)+fogPosition[10]*z)+fogPosition[11];
            x=a;y=b;z=c;
        }
        output[0]=(float)Math.sqrt((x*x+y*y)+z*z);
        output[1]=Math.max((float)Math.sqrt(x*x+z*z),Math.abs(y));
    }

    float fade(float spherical,float cylindrical) {
        return (1-Math.max(fog(spherical,environmentStart,environmentEnd),fog(cylindrical,distanceStart,distanceEnd)))*strength;
    }
    private static float fog(float distance,float start,float end) {
        if(distance<=start)return 0;
        if(distance>=end)return 1;
        return (distance-start)/(end-start);
    }

    /** Linear REPEAT, at texel centres, without an intermediate 8-bit sample.
     * rgba is caller-owned scratch space; fragment output is quantized later. */
    static void sample(BufferedImage texture,float u,float v,float[] rgba) {
        int width=texture.getWidth(),height=texture.getHeight();
        double x=(double)u*width-.5,y=(double)v*height-.5,fx=Math.floor(x),fy=Math.floor(y);
        int x0=wrap(fx,width),y0=wrap(fy,height),x1=(x0+1)%width,y1=(y0+1)%height;
        float ax=(float)(x-fx),ay=(float)(y-fy);
        int a=texture.getRGB(x0,y0),b=texture.getRGB(x1,y0),c=texture.getRGB(x0,y1),d=texture.getRGB(x1,y1);
        for(int channel=0;channel<4;channel++) {
            int shift=channel==3?24:16-channel*8;
            float ca=((a>>>shift)&255)/255f,cb=((b>>>shift)&255)/255f,cc=((c>>>shift)&255)/255f,cd=((d>>>shift)&255)/255f;
            float top=ca+(cb-ca)*ax,bottom=cc+(cd-cc)*ax;
            rgba[channel]=top+(bottom-top)*ay;
        }
    }
    private static int wrap(double floored,int size) {
        int value=(int)(floored%size);return value<0?value+size:value;
    }

    /** glint.fsh plus GLINT blending: source RGB is its own blend factor;
     * destination alpha survives. Vertex tint, actor fade, normals and the
     * entity lightmap do not participate in this shader. */
    int fragment(float[] rgba,float spherical,float cylindrical,int destination) {
        if(rgba[3]*modulator[3]<.1f)return destination;
        float fade=fade(spherical,cylindrical);int output=destination&0xff000000;
        for(int channel=0;channel<3;channel++) {
            int shift=16-channel*8;float source=rgba[channel]*modulator[channel]*fade;
            float color=source*source+((destination>>>shift)&255)/255f;
            int quantized=Math.max(0,Math.min(255,Math.round(color*255)));
            output|=quantized<<shift;
        }
        return output;
    }
}
