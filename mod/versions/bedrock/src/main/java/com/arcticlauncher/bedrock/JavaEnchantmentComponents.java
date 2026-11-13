package com.arcticlauncher.bedrock;

import com.google.gson.*;
import java.io.IOException;
import java.util.*;

import com.arcticlauncher.client.Lang;
/** Vanilla enchantment-component projection from the pinned native decoder's
 * copied inputs. It never supplies Java glint overrides or Item.isFoil overrides,
 * changes an inventory, or treats a component projection as native draw proof. */
public final class JavaEnchantmentComponents {
    static final String REFERENCE="java-26.2-bedrock-1.26.5203.0";
    // Pinned Enchant::Type order, matched to cached Java Enchantments keys.
    // These are resource names, not Java wire-protocol registry integers.
    private static final String[] NAMES={
        "protection","fire_protection","feather_falling","blast_protection","projectile_protection",
        "thorns","respiration","depth_strider","aqua_affinity","sharpness","smite","bane_of_arthropods",
        "knockback","fire_aspect","looting","efficiency","silk_touch","unbreaking","fortune","power",
        "punch","flame","infinity","luck_of_the_sea","lure","frost_walker","mending","binding_curse",
        "vanishing_curse","impaling","riptide","loyalty","channeling","multishot","piercing","quick_charge",
        "soul_speed","swift_sneak","wind_burst","density","breach","lunge"
    };
    private static final Set<String> KEYS;
    private static final Data EMPTY;
    static {
        Set<String> names=new HashSet<String>();for(String name:NAMES)names.add("minecraft:"+name);
        KEYS=Collections.unmodifiableSet(names);EMPTY=new Data(Collections.<String,Integer>emptyMap());
    }
    private JavaEnchantmentComponents() {}

    /** Portable owned component values. Level zero follows the checked Java
     * constructor/stream semantics, not its stricter JSON component codec.
     * Gson input is checked and replaced with a freshly derived immutable map. */
    public static final class Data {
        private int schema;
        private String reference;
        private Map<String,Integer> levels;
        private Data() {}
        private Data(Map<String,Integer> levels) {
            schema=1;reference=REFERENCE;
            this.levels=Collections.unmodifiableSortedMap(new TreeMap<String,Integer>(levels));
        }
        void validate() {
            if(schema!=1||!REFERENCE.equals(reference)||levels==null||levels.size()>NAMES.length)
                throw new IllegalArgumentException("invalid Java enchantment projection");
            for(Map.Entry<String,Integer> entry:levels.entrySet())if(!KEYS.contains(entry.getKey())||entry.getValue()==null||entry.getValue()<0||entry.getValue()>255)
                throw new IllegalArgumentException("invalid projected Java enchantment name/level");
        }
        /** Base Item.isFoil only. ItemStack.hasFoil still needs its explicit
         * component override and selected Item implementation to be resolved. */
        public boolean baseItemFoil() {validate();return !levels.isEmpty();}
        public Map<String,Integer> levels() {validate();return Collections.unmodifiableMap(levels);}
        boolean same(Data other) {
            return other!=null&&schema==other.schema&&Objects.equals(reference,other.reference)&&Objects.equals(levels,other.levels);
        }
    }
    static final class Projection {
        final Data data;
        final String reason;
        Projection(Data data,String reason){this.data=data;this.reason=reason;}
    }
    private static Projection unknown(String reason){return new Projection(null,reason);}

    /** Called after ItemSlot's raw shape/range validation. Native defaults and
     * ID narrowing follow the inspected decoder; raw fields remain untouched. */
    static Projection project(WorldFrame.ItemSlot slot) {
        if(slot.empty||Boolean.FALSE.equals(slot.enchanted))return new Projection(EMPTY,null);
        if(!Boolean.TRUE.equals(slot.enchanted))return unknown("native enchantment-list presence unavailable");
        if(Integer.valueOf(0).equals(slot.enchantmentListSize))return new Projection(EMPTY,null);
        if(slot.enchantmentEntries==null)return unknown("native enchantment entries unavailable");
        Map<String,Integer> levels=new TreeMap<String,Integer>();
        for(WorldFrame.EnchantmentEntry entry:slot.enchantmentEntries) {
            int id=entry.id==null?0:entry.id&255;
            if(id>=NAMES.length)continue; // Native ItemEnchants decoder skips IDs above 41.
            int level=entry.level==null?0:entry.level;
            if(level<0||level>255)return unknown("native enchantment level exceeds Java component range"); // i18n-ignore
            String name="minecraft:"+NAMES[id];
            // A map cannot preserve repeated native instances. Do not invent
            // first/last/max-level or duplicate-effect semantics.
            if(levels.containsKey(name))return unknown("duplicate native enchantment instances need conversion semantics");
            levels.put(name,level);
        }
        return new Projection(levels.isEmpty()?EMPTY:new Data(levels),null);
    }

    static void validateJson(JsonElement value) throws IOException {
        if(value==null||value.isJsonNull())return;
        if(!value.isJsonObject())throw new IOException("invalid recorded Java enchantment projection");
        JsonObject object=value.getAsJsonObject();
        if(object.entrySet().size()!=3||!object.has("schema")||!object.has("reference")||!object.has("levels"))
            throw new IOException("invalid recorded Java enchantment projection fields");
        integer(object.get("schema"),1,1);
        JsonElement reference=object.get("reference"),levels=object.get("levels");
        if(!reference.isJsonPrimitive()||!reference.getAsJsonPrimitive().isString()||!REFERENCE.equals(reference.getAsString())
                ||!levels.isJsonObject()||levels.getAsJsonObject().entrySet().size()>NAMES.length)
            throw new IOException("unsupported recorded Java enchantment projection");
        for(Map.Entry<String,JsonElement> entry:levels.getAsJsonObject().entrySet()) {
            if(!KEYS.contains(entry.getKey()))throw new IOException("unsupported recorded Java enchantment key");
            integer(entry.getValue(),0,255);
        }
    }
    private static void integer(JsonElement value,int min,int max) throws IOException {
        if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isNumber())throw new IOException("invalid recorded Java enchantment integer");
        double number=value.getAsDouble();long integer=value.getAsLong();
        if(!Double.isFinite(number)||number!=integer||integer<min||integer>max)throw new IOException("recorded Java enchantment integer out of range");
    }

    static JsonObject status(WorldFrame world) {
        int mapped=0,unmapped=0,foil=0,unknownGroups=0;
        Map<String,Integer> reasons=new TreeMap<String,Integer>();
        if(world!=null)for(WorldFrame.Player player:world.players) {
            if(player.equipment==null){unknownGroups+=2;continue;}
            for(WorldFrame.ItemSlot[] group:new WorldFrame.ItemSlot[][]{player.equipment.armor,player.equipment.hands}) {
                if(group==null){unknownGroups++;continue;}
                for(WorldFrame.ItemSlot slot:group) {
                    if(slot.javaEnchantments!=null){mapped++;if(slot.javaEnchantments.baseItemFoil())foil++;}
                    else {unmapped++;String reason=slot.javaEnchantmentError==null?"projection unavailable":slot.javaEnchantmentError; // i18n-ignore
                        Integer count=reasons.get(reason);reasons.put(reason,count==null?1:count+1);}
                }
            }
        }
        JsonObject state=new JsonObject();state.addProperty("reference",REFERENCE);
        state.addProperty("mappedSlots",mapped);state.addProperty("unmappedSlots",unmapped);state.addProperty("unknownGroups",unknownGroups);
        state.addProperty("baseItemFoilSlots",foil);JsonObject failures=new JsonObject();for(Map.Entry<String,Integer> entry:reasons.entrySet())failures.addProperty(entry.getKey(),entry.getValue());
        state.add("unavailableReasons",failures);state.addProperty("javaHasFoilOverridesMapped",false);
        state.addProperty("componentProjectionRuntimeVerified",false);state.addProperty("glintRenderingVerified",false);return state;
    }
}
