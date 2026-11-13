package com.arcticlauncher.bedrock;

import com.google.gson.*;
import java.io.*;
import java.nio.ByteBuffer;
import java.util.Arrays;

/** Explicit target-Java wing glint selection/uniforms, never an enchantment
 * inference, installed-game receipt or permission to suppress a native draw.
 * Unknown is null; selected=false is a distinct observed absence. */
final class GlintSample {
    static final int FLOATS=40;
    final boolean selected;
    private final float[] values;
    private GlintSample(boolean selected,float[] values) {
        if(selected!=(values!=null))throw new IllegalArgumentException("glint selection/uniform presence disagrees");
        if(values!=null) {
            if(values.length!=FLOATS)throw new IllegalArgumentException("glint uniform size differs");
            for(float value:values)if(!Float.isFinite(value))throw new IllegalArgumentException("non-finite glint input");
            if(values[33]<values[32]||values[35]<values[34]||values[36]<0||values[36]>1)
                throw new IllegalArgumentException("invalid glint fog/strength");
        }
        this.selected=selected;this.values=values==null?null:values.clone();
    }
    static GlintSample parse(JsonObject input) {
        if(input==null)return null;
        JsonElement schema=input.get("schema"),selected=input.get("selected");
        if(schema==null||!schema.isJsonPrimitive()||!schema.getAsJsonPrimitive().isNumber()||schema.getAsBigDecimal().compareTo(java.math.BigDecimal.ONE)!=0
                ||selected==null||!selected.isJsonPrimitive()||!selected.getAsJsonPrimitive().isBoolean())
            throw new IllegalArgumentException("invalid wing glint schema/selection");
        boolean active=selected.getAsBoolean();
        if(input.entrySet().size()!=(active?8:2))throw new IllegalArgumentException("unexpected wing glint fields");
        if(!active)return new GlintSample(false,null);
        float[] values=new float[FLOATS];
        array(input,"textureMatrix",values,0,16);array(input,"fogPosition",values,16,12);
        array(input,"colorModulator",values,28,4);array(input,"fogRanges",values,32,4);
        values[36]=number(input.get("glintAlpha"));array(input,"viewTranslation",values,37,3);
        return new GlintSample(true,values);
    }
    private static void array(JsonObject input,String key,float[] target,int offset,int size) {
        JsonElement value=input.get(key);
        if(value==null||!value.isJsonArray()||value.getAsJsonArray().size()!=size)
            throw new IllegalArgumentException("missing/invalid wing glint "+key);
        for(int i=0;i<size;i++)target[offset+i]=number(value.getAsJsonArray().get(i));
    }
    private static float number(JsonElement value) {
        if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isNumber()||!Float.isFinite(value.getAsFloat()))
            throw new IllegalArgumentException("invalid glint number");
        return value.getAsFloat();
    }
    boolean same(GlintSample other) {return other!=null&&selected==other.selected&&Arrays.equals(values,other.values);}
    GlintSample copy(){return new GlintSample(selected,values);}
    float[] values(){return values==null?null:values.clone();}
    void write(DataOutput output) throws IOException {
        output.writeByte(selected?2:1);if(selected)for(float value:values)output.writeFloat(value);
    }
    void write(ByteBuffer output) {
        output.put((byte)(selected?2:1));if(selected)for(float value:values)output.putFloat(value);
    }
    static GlintSample read(DataInput input) throws IOException {
        int state=input.readUnsignedByte();
        if(state!=1&&state!=2)throw new IOException("invalid recorded glint selection");
        float[] values=state==2?new float[FLOATS]:null;
        if(values!=null)for(int i=0;i<values.length;i++)values[i]=input.readFloat();
        try{return new GlintSample(state==2,values);}catch(IllegalArgumentException invalid){throw new IOException("invalid recorded glint inputs",invalid);}
    }
}
