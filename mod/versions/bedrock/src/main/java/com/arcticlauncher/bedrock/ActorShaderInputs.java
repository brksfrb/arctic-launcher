package com.arcticlauncher.bedrock;

import com.google.gson.*;
import java.io.*;

/** Owned named CPU constants at a selected native layer draw. Layout: overlay
 * RGBA, tile light RGBA, tile intensity XY, block light RGBA, change RGBA,
 * glint RGBA, UV animation XYZW, UV offset XY, UV rotation XY, glint UV scale XY.
 * Optional material context: multiplicative tint RGBA, fog RGBA, fog control XY,
 * render distance and HUD opacity, copied from the same native draw scope.
 * Optional material paths fingerprint the guarded declared stage paths only.
 * Shader execution, material identity and native resource ownership are separate. */
final class ActorShaderInputs {
    private static final int COUNT=32;
    private static final int CONTEXT_COUNT=12;
    private final float[] values;
    private final float[] materialContext;
    private final MaterialPaths materialPaths;
    private ActorShaderInputs(float[] values) {
        this(values,null);
    }
    private ActorShaderInputs(float[] values,float[] context) {
        this(values,context,null);
    }
    private ActorShaderInputs(float[] values,float[] context,MaterialPaths paths) {
        this.values=values==null?null:values.clone();materialContext=context==null?null:context.clone();materialPaths=paths;validate();
    }
    void validate() {
        if(values==null||values.length!=COUNT)throw new IllegalArgumentException("native actor shader inputs need 32 values");
        for(float value:values)if(!Float.isFinite(value)||Math.abs(value)>1_000_000_000f)
            throw new IllegalArgumentException("invalid native actor shader input");
        if(materialContext!=null) {
            if(materialContext.length!=CONTEXT_COUNT)throw new IllegalArgumentException("native material context needs 12 values");
            for(float value:materialContext)if(!Float.isFinite(value)||Math.abs(value)>1_000_000_000f)
                throw new IllegalArgumentException("invalid native material context");
        }
        if(materialPaths!=null)materialPaths.validate();
    }
    boolean hasMaterialContext(){validate();return materialContext!=null;}
    boolean hasMaterialPaths(){validate();return materialPaths!=null;}
    boolean hasMaterialVariation(){validate();return materialPaths!=null&&materialPaths.variationSha256!=null;}
    ActorShaderInputs copy(){validate();return new ActorShaderInputs(values,materialContext,materialPaths);}
    /** A legacy override leaves the checked live context and path declarations
     * in place. A new recording may retain those actual playback observations. */
    ActorShaderInputs withUnchangedContext(ActorShaderInputs source) {
        validate();if(source!=null)source.validate();
        return new ActorShaderInputs(values,materialContext!=null?materialContext:source==null?null:source.materialContext,
            materialPaths!=null?materialPaths.withUnchangedVariation(source==null?null:source.materialPaths):source==null?null:source.materialPaths);
    }
    static boolean same(ActorShaderInputs a,ActorShaderInputs b) {
        if(a==null||b==null)return a==b;a.validate();b.validate();
        for(int i=0;i<COUNT;i++)if(Float.floatToIntBits(a.values[i])!=Float.floatToIntBits(b.values[i]))return false;
        if(a.materialContext==null||b.materialContext==null){if(a.materialContext!=b.materialContext)return false;}
        else for(int i=0;i<CONTEXT_COUNT;i++)if(Float.floatToIntBits(a.materialContext[i])!=Float.floatToIntBits(b.materialContext[i]))return false;
        return MaterialPaths.same(a.materialPaths,b.materialPaths);
    }
    static ActorShaderInputs owned(JsonElement value) {
        if(value==null||value.isJsonNull())return null;
        if(!value.isJsonObject()||value.getAsJsonObject().entrySet().size()<1||value.getAsJsonObject().entrySet().size()>3)
            throw new IllegalArgumentException("invalid owned native actor shader inputs");
        for(String key:keys(value.getAsJsonObject()))if(!"values".equals(key)&&!"materialContext".equals(key)&&!"materialPaths".equals(key))
            throw new IllegalArgumentException("unknown owned native actor shader input");
        float[] values=vector(value.getAsJsonObject().get("values"),COUNT);
        JsonElement context=value.getAsJsonObject().get("materialContext");
        return new ActorShaderInputs(values,context==null||context.isJsonNull()?null:vector(context,CONTEXT_COUNT),
            MaterialPaths.owned(value.getAsJsonObject().get("materialPaths")));
    }
    private static float[] vector(JsonElement array,int count) {
        if(array==null||!array.isJsonArray()||array.getAsJsonArray().size()!=count)
            throw new IllegalArgumentException("missing owned native actor shader values");
        float[] values=new float[count];
        for(int i=0;i<count;i++) {
            JsonElement channel=array.getAsJsonArray().get(i);
            if(!channel.isJsonPrimitive()||!channel.getAsJsonPrimitive().isNumber())
                throw new IllegalArgumentException("native actor shader input must be numeric");
            values[i]=channel.getAsFloat();
        }
        return values;
    }
    // UV wrap/rotation and discontinuous material switches need their native
    // interpolation rules. An interior interval is known only if unchanged.
    static ActorShaderInputs between(ActorShaderInputs a,ActorShaderInputs b) {
        return a!=null&&same(a,b)?a.copy():null;
    }
    void write(DataOutput output) throws IOException {
        write(output,false);
    }
    void write(DataOutput output,boolean context) throws IOException {
        write(output,context,false);
    }
    void write(DataOutput output,boolean context,boolean paths) throws IOException {
        write(output,context,paths,false);
    }
    void write(DataOutput output,boolean context,boolean paths,boolean variation) throws IOException {
        validate();for(float value:values)output.writeFloat(value);
        if(context){output.writeBoolean(materialContext!=null);if(materialContext!=null)for(float value:materialContext)output.writeFloat(value);}
        if(paths){output.writeBoolean(materialPaths!=null);if(materialPaths!=null)materialPaths.write(output,variation);}
    }
    static ActorShaderInputs read(DataInput input) throws IOException {
        return read(input,false);
    }
    static ActorShaderInputs read(DataInput input,boolean context) throws IOException {
        return read(input,context,false);
    }
    static ActorShaderInputs read(DataInput input,boolean context,boolean paths) throws IOException {
        return read(input,context,paths,false);
    }
    static ActorShaderInputs read(DataInput input,boolean context,boolean paths,boolean variation) throws IOException {
        float[] values=new float[COUNT];for(int i=0;i<COUNT;i++)values[i]=input.readFloat();
        float[] materialContext=null;
        if(context) {
            int present=input.readUnsignedByte();if(present>1)throw new IOException("invalid recorded material context presence");
            if(present==1){materialContext=new float[CONTEXT_COUNT];for(int i=0;i<CONTEXT_COUNT;i++)materialContext[i]=input.readFloat();}
        }
        MaterialPaths materialPaths=null;
        if(paths) {
            int present=input.readUnsignedByte();if(present>1)throw new IOException("invalid recorded material paths presence");
            if(present==1)materialPaths=MaterialPaths.read(input,variation);
        }
        try{return new ActorShaderInputs(values,materialContext,materialPaths);}catch(IllegalArgumentException invalid){throw new IOException("invalid recorded actor shader inputs",invalid);}
    }
    /** Immutable metadata: no addresses, path text, shader assets or handles. */
    private static final class MaterialPaths {
        private final String sha256;
        private final boolean[] stages;
        private final String variationSha256;
        private MaterialPaths(String hash,boolean[] stages,String variation){sha256=hash;this.stages=stages==null?null:stages.clone();variationSha256=variation;validate();}
        private void validate(){if(sha256==null||!sha256.matches("[0-9a-f]{64}")||stages==null||stages.length!=3
                ||variationSha256!=null&&!variationSha256.matches("[0-9a-f]{64}"))
            throw new IllegalArgumentException("invalid native material paths fingerprint");}
        private static boolean same(MaterialPaths a,MaterialPaths b){return a==null||b==null?a==b:a.sha256.equals(b.sha256)
            &&java.util.Arrays.equals(a.stages,b.stages)&&java.util.Objects.equals(a.variationSha256,b.variationSha256);}
        private MaterialPaths withUnchangedVariation(MaterialPaths source) {
            if(source==null)return this;
            if(!sha256.equals(source.sha256)||!java.util.Arrays.equals(stages,source.stages)
                    ||variationSha256!=null&&!variationSha256.equals(source.variationSha256))
                throw new IllegalArgumentException("replayed material declaration differs from its live source");
            return variationSha256!=null?this:new MaterialPaths(sha256,stages,source.variationSha256);
        }
        private static MaterialPaths owned(JsonElement value) {
            if(value==null||value.isJsonNull())return null;
            if(!value.isJsonObject()||value.getAsJsonObject().entrySet().size()<2||value.getAsJsonObject().entrySet().size()>3)throw new IllegalArgumentException("invalid owned material paths fingerprint");
            for(String key:keys(value.getAsJsonObject()))if(!"sha256".equals(key)&&!"stages".equals(key)&&!"variationSha256".equals(key))
                throw new IllegalArgumentException("unknown owned material paths field");
            JsonElement hash=value.getAsJsonObject().get("sha256"),stages=value.getAsJsonObject().get("stages");
            if(hash==null||!hash.isJsonPrimitive()||!hash.getAsJsonPrimitive().isString()
                    ||stages==null||!stages.isJsonArray()||stages.getAsJsonArray().size()!=3)throw new IllegalArgumentException("invalid owned material stage paths");
            boolean[] flags=new boolean[3];
            for(int i=0;i<3;i++) {
                JsonElement flag=stages.getAsJsonArray().get(i);
                if(!flag.isJsonPrimitive()||!flag.getAsJsonPrimitive().isBoolean())throw new IllegalArgumentException("material stage path presence must be boolean");
                flags[i]=flag.getAsBoolean();
            }
            JsonElement variation=value.getAsJsonObject().get("variationSha256");
            if(variation!=null&&!variation.isJsonNull()&&(!variation.isJsonPrimitive()||!variation.getAsJsonPrimitive().isString()))
                throw new IllegalArgumentException("material variation fingerprint must be a string");
            return new MaterialPaths(hash.getAsString(),flags,variation==null||variation.isJsonNull()?null:variation.getAsString());
        }
        private static void writeHash(DataOutput output,String hash) throws IOException {
            for(int i=0;i<32;i++)output.writeByte(Integer.parseInt(hash.substring(i*2,i*2+2),16));
        }
        private static String readHash(DataInput input) throws IOException {
            StringBuilder hash=new StringBuilder(64);
            for(int i=0;i<32;i++){int value=input.readUnsignedByte();hash.append(Character.forDigit(value>>>4,16)).append(Character.forDigit(value&15,16));}
            return hash.toString();
        }
        private void write(DataOutput output,boolean variation) throws IOException {
            validate();writeHash(output,sha256);
            int mask=0;for(int i=0;i<3;i++)if(stages[i])mask|=1<<i;output.writeByte(mask);
            if(variation){output.writeBoolean(variationSha256!=null);if(variationSha256!=null)writeHash(output,variationSha256);}
        }
        private static MaterialPaths read(DataInput input,boolean variation) throws IOException {
            String hash=readHash(input);
            int mask=input.readUnsignedByte();if(mask>7)throw new IOException("invalid recorded material stage path mask");
            String variationHash=null;
            if(variation){int present=input.readUnsignedByte();if(present>1)throw new IOException("invalid recorded material variation presence");if(present==1)variationHash=readHash(input);}
            return new MaterialPaths(hash,new boolean[]{(mask&1)!=0,(mask&2)!=0,(mask&4)!=0},variationHash);
        }
    }
    /** The object's keys (Gson 2.2.4, the game's own, has no {@code keySet}). */
    private static java.util.List<String> keys(JsonObject object) {
        java.util.List<String> out=new java.util.ArrayList<String>();
        for(java.util.Map.Entry<String,JsonElement> entry:object.entrySet())out.add(entry.getKey());
        return out;
    }
}
