package com.arcticlauncher.bedrock;

import com.google.gson.*;
import java.io.IOException;
import java.util.*;

import com.arcticlauncher.client.Lang;
/** Java 26.2 foil branch ordering over explicit adapter inputs. This does not
 * discover Java components in native memory or convert native forcedGlint into
 * an observed Java override. Normal native snapshots currently omit these inputs. */
public final class JavaItemFoil {
    private static final String REFERENCE="java-26.2";
    private static final Set<String> OVERRIDES=Collections.unmodifiableSet(new HashSet<String>(Arrays.asList("unknown","absent","true","false")));
    private static final Set<String> PREDICATES=Collections.unmodifiableSet(new HashSet<String>(Arrays.asList("unknown","base","vanilla-elytra","true","false")));
    private JavaItemFoil() {}

    /** An adapter must own the source facts before supplying a known branch.
     * true/false predicates are copied results of the selected Java item
     * predicate, not a claim that its implementation is always constant.
     * base declares the selected base implementation; vanilla-elytra names
     * the independently inspected default registration for minecraft:elytra. */
    public static final class Input {
        private int schema;
        private String reference,componentOverride,itemPredicate;
        private Input() {}
        public static Input of(String componentOverride,String itemPredicate) {
            Input value=new Input();value.schema=1;value.reference=REFERENCE;
            value.componentOverride=componentOverride;value.itemPredicate=itemPredicate;
            value.validateStates();return value;
        }
        private void validateStates() {
            if(schema!=1||!REFERENCE.equals(reference)||!OVERRIDES.contains(componentOverride)||!PREDICATES.contains(itemPredicate))
                throw new IllegalArgumentException("invalid Java foil input reference/state");
        }
        void validate(String item) {
            validateStates();
            if("vanilla-elytra".equals(itemPredicate)&&!"minecraft:elytra".equals(item))
                throw new IllegalArgumentException("vanilla Elytra foil input belongs to another item");
        }
        Input copy(){return of(componentOverride,itemPredicate);}
        boolean matches(Input actual) {
            // Unknown recorded facts make no assertion. Known absence remains
            // different from unknown, true and false in the current snapshot.
            return ("unknown".equals(componentOverride)||actual!=null&&componentOverride.equals(actual.componentOverride))
                &&("unknown".equals(itemPredicate)||actual!=null&&itemPredicate.equals(actual.itemPredicate));
        }
    }
    static final class Selection {
        final Boolean value;
        final String reason;
        Selection(Boolean value,String reason){this.value=value;this.reason=reason;}
    }
    private static Selection unknown(String reason){return new Selection(null,reason);}

    /** Empty equipment has no foil draw. Every nonempty decision follows
     * ItemStack.hasFoil: component override first, then selected Item.isFoil.
     * A known override can resolve without enchantments or an item predicate. */
    static Selection select(WorldFrame.ItemSlot slot) {
        if(slot.empty)return new Selection(Boolean.FALSE,null);
        Input input=slot.javaFoilInputs;
        if(input==null||"unknown".equals(input.componentOverride))return unknown("Java glint component override unavailable");
        if("true".equals(input.componentOverride))return new Selection(Boolean.TRUE,null);
        if("false".equals(input.componentOverride))return new Selection(Boolean.FALSE,null);
        if("true".equals(input.itemPredicate))return new Selection(Boolean.TRUE,null);
        if("false".equals(input.itemPredicate))return new Selection(Boolean.FALSE,null);
        if("unknown".equals(input.itemPredicate))return unknown("selected Java item foil predicate unavailable");
        if(slot.javaEnchantments==null)return unknown(Lang.t("platform.java_enchantment_component_projection"));
        return new Selection(slot.javaEnchantments.baseItemFoil(),null);
    }

    static void validateJson(JsonElement value) throws IOException {
        if(value==null||value.isJsonNull())return;
        if(!value.isJsonObject())throw new IOException("invalid recorded Java foil inputs");
        JsonObject object=value.getAsJsonObject();
        if(object.entrySet().size()!=4||!object.has("schema")||!object.has("reference")||!object.has("componentOverride")||!object.has("itemPredicate"))
            throw new IOException("invalid recorded Java foil input fields");
        JsonElement schema=object.get("schema");
        if(!schema.isJsonPrimitive()||!schema.getAsJsonPrimitive().isNumber()||schema.getAsDouble()!=1||schema.getAsLong()!=1)
            throw new IOException("unsupported recorded Java foil schema");
        if(!REFERENCE.equals(string(object,"reference"))||!OVERRIDES.contains(string(object,"componentOverride"))||!PREDICATES.contains(string(object,"itemPredicate")))
            throw new IOException("unsupported recorded Java foil input state");
    }
    private static String string(JsonObject object,String key) throws IOException {
        JsonElement value=object.get(key);
        if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isString())throw new IOException("invalid recorded Java foil string: "+key);
        return value.getAsString();
    }

    static JsonObject status(WorldFrame world) {
        int yes=0,no=0,unknown=0,unknownGroups=0;Map<String,Integer> reasons=new TreeMap<String,Integer>();
        if(world!=null)for(WorldFrame.Player player:world.players) {
            if(player.equipment==null){unknownGroups+=2;continue;}
            for(WorldFrame.ItemSlot[] group:new WorldFrame.ItemSlot[][]{player.equipment.armor,player.equipment.hands}) {
                if(group==null){unknownGroups++;continue;}
                for(WorldFrame.ItemSlot slot:group) {
                    if(Boolean.TRUE.equals(slot.javaFoil))yes++;
                    else if(Boolean.FALSE.equals(slot.javaFoil))no++;
                    else {unknown++;String reason=slot.javaFoilError==null?"foil selection unavailable":slot.javaFoilError;
                        Integer count=reasons.get(reason);reasons.put(reason,count==null?1:count+1);}
                }
            }
        }
        JsonObject state=new JsonObject();state.addProperty("reference",REFERENCE);
        state.addProperty("foilSlots",yes);state.addProperty("noFoilSlots",no);state.addProperty("unknownSlots",unknown);state.addProperty("unknownGroups",unknownGroups);
        JsonObject unavailable=new JsonObject();for(Map.Entry<String,Integer> entry:reasons.entrySet())unavailable.addProperty(entry.getKey(),entry.getValue());
        state.add("unavailableReasons",unavailable);state.addProperty("nativeOverrideProducerAvailable",false);
        state.addProperty("runtimeSelectionVerified",false);state.addProperty("glintRenderingVerified",false);return state;
    }
}
