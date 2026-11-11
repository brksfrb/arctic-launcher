package com.arcticlauncher.bedrock;

import java.io.*;
import com.google.gson.JsonElement;

/** Copied Java layer decisions, separate from skin ModelPart visibility and
 * native draw culling. Missing inputs remain unknown, never false by default. */
public final class JavaLayerVisibility {
    static final class Inputs {
        JsonElement schema,spectator,invisible,showCape;
    }
    private final int known,flags;
    private JavaLayerVisibility(int known,int flags){this.known=known;this.flags=flags;}
    public static JavaLayerVisibility of(boolean spectator,boolean invisible,boolean showCape) {
        return partial(spectator,invisible,showCape);
    }
    /** Unknown facts do not erase independent observations. Partial values
     * can deny a layer but cannot establish every layer's positive eligibility. */
    public static JavaLayerVisibility partial(Boolean spectator,Boolean invisible,Boolean showCape) {
        int known=(spectator==null?0:1)|(invisible==null?0:2)|(showCape==null?0:4);
        int flags=(Boolean.TRUE.equals(spectator)?1:0)|(Boolean.TRUE.equals(invisible)?2:0)|(Boolean.TRUE.equals(showCape)?4:0);
        return known==0?null:new JavaLayerVisibility(known,flags);
    }
    public boolean complete(){return known==7;}
    public boolean layersKnown(){return (known&1)!=0;}
    public boolean cosmeticsKnown(){return (known&3)==3;}
    public boolean capeKnown(){return known==7;}
    /** Explicitly copied invisibility, independent of layer eligibility. */
    public boolean invisibleObserved(){return (known&2)!=0&&(flags&2)!=0;}
    public boolean layersShown(){return (flags&1)==0;}
    public boolean cosmeticsShown(){return layersShown()&&(flags&2)==0;}
    public boolean capeShown(){return cosmeticsShown()&&((known&4)==0||(flags&4)!=0);}
    boolean same(JavaLayerVisibility other){return other!=null&&known==other.known&&flags==other.flags;}
    static JavaLayerVisibility between(JavaLayerVisibility a,JavaLayerVisibility b) {
        if(a==null||b==null)return null;
        // Retain each independently unchanged discrete fact. A change in
        // invisibility does not erase an unchanged spectator observation.
        int known=a.known&b.known&~(a.flags^b.flags);
        return known==0?null:new JavaLayerVisibility(known,a.flags&known);
    }
    static JavaLayerVisibility parse(Inputs input) {
        if(input==null)return null;
        if(input.schema==null||!input.schema.isJsonPrimitive()||!input.schema.getAsJsonPrimitive().isNumber()
                ||input.schema.getAsDouble()!=1||input.schema.getAsLong()!=1)
            throw new IllegalArgumentException("invalid Java layer visibility schema");
        return partial(flag(input.spectator),flag(input.invisible),flag(input.showCape));
    }
    private static Boolean flag(JsonElement value) {
        if(value==null||value.isJsonNull())return null;
        if(!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isBoolean())
            throw new IllegalArgumentException("Java layer facts must be explicit booleans");
        return value.getAsBoolean();
    }
    static void write(DataOutput output,JavaLayerVisibility value) throws IOException {
        output.writeByte(value==null?0:1);if(value!=null){output.writeByte(value.known);output.writeByte(value.flags);}
    }
    static JavaLayerVisibility read(DataInput input) throws IOException {
        int present=input.readUnsignedByte();if(present==0)return null;
        if(present!=1)throw new IOException("invalid recorded Java layer visibility presence");
        int known=input.readUnsignedByte(),flags=input.readUnsignedByte();
        if(known<1||known>7||(flags&~known)!=0)throw new IOException("invalid recorded Java layer visibility masks");
        return new JavaLayerVisibility(known,flags);
    }
}
