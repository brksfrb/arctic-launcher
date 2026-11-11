package com.arcticlauncher.bedrock;

import com.google.gson.*;
import java.io.IOException;

/** Java 26.2 CapeLayer equipment branches over copied selected-layer facts.
 * Neither a native armor ID nor gliding attests Java EQUIPPABLE asset layers. */
public final class JavaCapeLayer {
    private static final String REFERENCE="java-26.2";
    public enum Selection { UNKNOWN, PLAIN, OFFSET, HIDDEN }
    private JavaCapeLayer() {}

    /** Copied results of hasLayer(chest,WINGS/HUMANOID). Null is unknown.
     * An adapter owns component/asset lookup before supplying either fact. */
    public static final class Input {
        private int schema;
        private String reference;
        private Boolean wings,humanoid;
        private Input() {}
        public static Input of(Boolean wings,Boolean humanoid) {
            Input input=new Input();input.schema=1;input.reference=REFERENCE;
            input.wings=wings;input.humanoid=humanoid;return input;
        }
        void validate() {
            if(schema!=1||!REFERENCE.equals(reference))throw new IllegalArgumentException("invalid Java cape layer input reference");
        }
        Input copy(){return of(wings,humanoid);}
        boolean matches(Input actual) {
            return (wings==null||actual!=null&&wings.equals(actual.wings))
                &&(humanoid==null||actual!=null&&humanoid.equals(actual.humanoid));
        }
    }

    static Selection select(WorldFrame.Equipment equipment) {
        return equipment==null||equipment.armor==null?Selection.UNKNOWN:select(equipment.armor[1]);
    }
    static Selection select(WorldFrame.ItemSlot chest) {
        if(chest==null)return Selection.UNKNOWN;
        if(chest.empty)return Selection.PLAIN;
        Input input=chest.javaCapeLayers;
        if(input==null)return Selection.UNKNOWN;
        if(Boolean.TRUE.equals(input.wings))return Selection.HIDDEN;
        if(!Boolean.FALSE.equals(input.wings)||input.humanoid==null)return Selection.UNKNOWN;
        return input.humanoid?Selection.OFFSET:Selection.PLAIN;
    }

    /** CapeLayer translates the entity root before the body's model pose.
     * Keep the default non-FMA JOML translateGeneric rightward float grouping.
     * This does not infer the native pose stack's original property flags. */
    static float[] offsetRoot(float[] root) {
        float[] shifted=root.clone();
        for(int row=0;row<3;row++) {
            int i=row*4;
            shifted[i+3]=root[i]*0f+(root[i+1]*-.05312500149011612f+(root[i+2]*.06875000149011612f+root[i+3]));
        }
        return shifted;
    }

    static void validateJson(JsonElement value) throws IOException {
        if(value==null||value.isJsonNull())return;
        if(!value.isJsonObject())throw new IOException("invalid recorded Java cape layers");
        JsonObject input=value.getAsJsonObject();
        if(input.entrySet().size()!=4||!input.has("schema")||!input.has("reference")||!input.has("wings")||!input.has("humanoid"))
            throw new IOException("invalid recorded Java cape layer fields");
        JsonElement schema=input.get("schema"),reference=input.get("reference");
        if(!schema.isJsonPrimitive()||!schema.getAsJsonPrimitive().isNumber()||schema.getAsDouble()!=1||schema.getAsLong()!=1
                ||!reference.isJsonPrimitive()||!reference.getAsJsonPrimitive().isString()||!REFERENCE.equals(reference.getAsString()))
            throw new IOException("unsupported recorded Java cape layer reference");
        for(String key:new String[]{"wings","humanoid"}) {
            JsonElement fact=input.get(key);
            if(!fact.isJsonNull()&&(!fact.isJsonPrimitive()||!fact.getAsJsonPrimitive().isBoolean()))
                throw new IOException("invalid recorded Java cape layer flag");
        }
    }

    static JsonObject status(WorldFrame world) {
        int[] counts=new int[Selection.values().length];
        if(world!=null)for(WorldFrame.Player player:world.players)counts[select(player.equipment).ordinal()]++;
        JsonObject value=new JsonObject();value.addProperty("reference",REFERENCE);
        for(Selection selection:Selection.values())value.addProperty(selection.name().toLowerCase(java.util.Locale.ROOT),counts[selection.ordinal()]);
        value.addProperty("nativeLayerInputProducerAvailable",false);value.addProperty("armoredNativeCapeReplacementVerified",false);
        value.addProperty("runtimeSelectionVerified",false);return value;
    }
}
