package com.arcticlauncher.bedrock;

import java.io.*;
import java.util.Arrays;

/** Explicit visible-player entityTranslucent selection and its actual selected
 * overlay texel. Ghost/outline/dissolve variants are not inferred from it. */
public final class JavaSkinMaterial {
    static final class Inputs {int schema;String kind;float[] overlayColor;}
    private final float[] overlay;
    private JavaSkinMaterial(float[] values) {
        WorldFrame.color(values);
        if(values==null)throw new IllegalArgumentException("skin material needs an explicit overlay texel");
        overlay=values.clone();
    }
    public static JavaSkinMaterial entityTranslucent(float[] selectedOverlay) {return new JavaSkinMaterial(selectedOverlay);}
    public float[] overlayColor(){return overlay.clone();}
    static JavaSkinMaterial parse(Inputs input) {
        if(input==null)return null;
        if(input.schema!=1||!"entity-translucent".equals(input.kind))throw new IllegalArgumentException("unsupported explicit skin material");
        return entityTranslucent(input.overlayColor);
    }
    boolean same(JavaSkinMaterial other){return other!=null&&Arrays.equals(overlay,other.overlay);}
    static void write(DataOutput output,JavaSkinMaterial material) throws IOException {
        output.writeByte(material==null?0:1);
        if(material!=null)for(float value:material.overlay)output.writeFloat(value);
    }
    static JavaSkinMaterial read(DataInput input) throws IOException {
        int kind=input.readUnsignedByte();if(kind==0)return null;
        if(kind!=1)throw new IOException("unsupported recorded skin material");
        float[] overlay=new float[4];for(int i=0;i<4;i++)overlay[i]=input.readFloat();
        try{return entityTranslucent(overlay);}catch(IllegalArgumentException invalid){throw new IOException("invalid recorded skin overlay",invalid);}
    }
}
