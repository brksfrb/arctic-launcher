package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.Gson;
import java.awt.image.BufferedImage;
import java.util.*;

/** Owned, checked engine snapshot. Supplying a snapshot is an engine adapter's
 * job; headless fixtures are never treated as evidence of Minecraft integration. */
public final class WorldFrame {
    private static final Gson GSON=new Gson();
    public int schema,width,height;
    public long sequence,clockMs,dayTime;
    public String session,server,dimension,localPlayer;
    public float[] viewProjection;
    /** Optional explicit Java entity shader cohort for this observation.
     * No native constant name, dimension or packed light implies this data. */
    public JavaEntityShader.Inputs javaEntityShaderInputs;
    private transient JavaEntityShader entityShader;
    /** Optional raw camera pair, copied with viewProjection in the same native
     * world frame. Older providers cannot authorize equipment layering. */
    public ProjectionContext projectionContext;
    public double[] camera;
    /** Declared coordinate space of model roots/viewProjection. Null retains
     * projected-depth sorting for older providers that did not declare it. */
    public Boolean cameraRelative;
    public Player[] players;
    /** Copied native timing diagnostics; never drives the engine. */
    public com.google.gson.JsonObject nativeTiming;
    private transient NativeShaderClock shaderClock;
    NativeShaderClock shaderClock(){return shaderClock;}
    public ResourceStack itemResources;
    private transient float[] depth;
    private transient boolean geometryOnly;
    private transient float[] equipmentProjection;
    float[] copyEquipmentProjection(){return equipmentProjection==null?null:equipmentProjection.clone();}
    private transient CosmeticClock.Sample replayCosmeticSample;

    /** Cape frames and cosmetic idle motion use the export's recorded time.
     * The native tick clock and live emote start times remain independent. */
    public long cosmeticTimeMillis() {return replayCosmeticSample==null?clockMs:replayCosmeticSample.timeMs;}
    double cosmeticTimeExactMillis() {return replayCosmeticSample==null?clockMs:replayCosmeticSample.exactMillis();}
    Long replayCosmeticTimeMillis() {return replayCosmeticSample==null?null:replayCosmeticSample.timeMs;}
    CosmeticClock.Sample replayCosmeticSample(){return replayCosmeticSample;}
    /** The raw affine view which was checked against this actual combined
     * projection. No decomposition of viewProjection or camera-angle guess. */
    float[] copyModelView(float[] usedProjection) {
        if(projectionContext==null||!Arrays.equals(viewProjection,usedProjection))return null;
        return projectionContext.checkedView(usedProjection,cameraRelative);
    }
    JavaQuadSorting copyQuadSorting(float[] usedProjection) {
        if(projectionContext==null||!Arrays.equals(viewProjection,usedProjection))return null;
        return projectionContext.checkedQuadSorting(usedProjection,cameraRelative);
    }

    public static final class ProjectionContext {
        public String type,clipDepth;
        public float[] view,matrix;
        public Boolean reverseZ;
        private transient float[] checkedView,checkedViewProjection;
        private transient String checkedType;
        private float[] checkedView(float[] expected,Boolean cameraRelative) {
            if(!Boolean.TRUE.equals(cameraRelative)||checkedView==null||!Arrays.equals(expected,checkedViewProjection))return null;
            return checkedView.clone();
        }
        private JavaQuadSorting checkedQuadSorting(float[] expected,Boolean cameraRelative) {
            if(!Boolean.TRUE.equals(cameraRelative)||checkedType==null||!Arrays.equals(expected,checkedViewProjection))return null;
            return JavaQuadSorting.forProjection(checkedType);
        }
        private float[] equipment(float[] expected,Boolean cameraRelative) {
            String ownedType=type;Boolean ownedReverseZ=reverseZ;
            if(!Boolean.TRUE.equals(cameraRelative)||!"negative_one_to_one".equals(clipDepth)||ownedReverseZ==null
                    ||!("perspective".equals(ownedType)||"orthographic".equals(ownedType)))
                throw new IllegalArgumentException("invalid native projection context");
            finite(view,16,1_000_000);finite(matrix,16,1_000_000);
            float[] ownedView=view.clone(),ownedProjection=matrix.clone();
            for(int i:new int[]{3,7,11})if(Math.abs(ownedView[i])>0.00001f)
                throw new IllegalArgumentException("native view is not affine");
            if(Math.abs(ownedView[15]-1)>0.00001f
                    ||"perspective".equals(ownedType)&&(Math.abs(ownedProjection[11])<.5f||Math.abs(ownedProjection[15])>.00001f)
                    ||"orthographic".equals(ownedType)&&(Math.abs(ownedProjection[11])>.00001f||Math.abs(ownedProjection[15]-1)>.00001f))
                throw new IllegalArgumentException("native projection type disagrees with its matrix");
            float[] original=JavaLayerProjection.combine(ownedView,ownedProjection,ownedType,ownedReverseZ,false);
            // Numeric float equality tolerates only the sign of zero. A pair
            // from another camera frame must not silently layer these actors.
            for(int i=0;i<16;i++)if(original[i]!=expected[i])
                throw new IllegalArgumentException("native projection pair disagrees with world camera");
            float[] equipment=JavaLayerProjection.combine(ownedView,ownedProjection,ownedType,ownedReverseZ,true);
            finite(equipment,16,1e8f);
            checkedView=ownedView;checkedViewProjection=expected.clone();checkedType=ownedType;
            return equipment;
        }
    }

    public static final class Player {
        public String id,name,skin,cape;
        /** Original observed game appearance, retained across a replacement draw. */
        public String nativeSkin;
        /** Original RGBA from this draw's checked standard skin description. */
        public float[] nativeSkinColor;
        /** Optional Java packed block/sky light copied for this actor/frame.
         * Native TILE_LIGHT_COLOR is not an interchangeable representation. */
        public Integer packedLight;
        /** Target Java material selection and selected body overlay. Neither
         * native skin alpha nor a cosmetic NO_OVERLAY texel establishes it. */
        public JavaSkinMaterial.Inputs javaSkinMaterial;
        transient JavaSkinMaterial skinMaterial;
        public JavaSkinVisibility.Inputs javaSkinVisibility;
        transient JavaSkinVisibility skinVisibility;
        public JavaLayerVisibility.Inputs javaLayerVisibility;
        transient JavaLayerVisibility layerVisibility;
        public DrawColor nativeCapeDrawColor,nativeWingDrawColor;
        /** Explicit target inputs only. Native foil/glow metadata cannot
         * establish the selected Java glint matrix/strength/clock/fog. */
        public com.google.gson.JsonObject javaWingGlint;
        transient GlintSample wingGlint;
        /** Actual copied native cape description after a leased replay write. */
        public CapeReplayColor nativeCapeReplayColor;
        public CapeReplayColor nativeWingReplayColor;
        public String nativeCape,capeHash;
        public Boolean nativeSlim;
        public boolean nativeCapeSuppressed,nativeCapeHidden;
        public String modelSource;
        public CapePhysics.Motion capeMotion;
        public CapePhysics.Snapshot capeState;
        /** Existing schema field is the active native gliding flag. */
        public boolean elytra;
        /** Null when chest identity is unavailable; gliding does not imply a
         * readable inventory item and standing does not imply unequipped. */
        public Boolean elytraEquipped;
        public String chestItem;
        public Equipment equipment;
        /** Target math from the same completed native skin pose. This is not
         * a receipt that native armor/held meshes consumed those matrices. */
        public EmotePose emotePose;
        public AttachmentSample attachmentPoses;
        /** Game-processed local input; absent when its native copy is unavailable. */
        public Movement movement;
        /** Feet position, yaw, pitch. */
        public double[] position;
        /** Engine model-to-world transform; Java player model coordinates, in blocks. */
        public float[] model;
        public float[][] partPivots,partRotations;
        /** Six rigid part-to-model transforms, row-major 3x4, in blocks. */
        public float[][] partMatrices;
        /** Observed left/right wing-to-model transforms, including native
         * nonuniform scale. Observation alone does not authorize a world draw. */
        public float[][] wingMatrices;
        public String wingTexture,wingHash;
        /** Only an owned suppressed native wing draw may set this receipt. */
        public boolean replaceElytra;
        /** Missing in v1 fixtures means all six parts; live adapters attest a mask. */
        public Integer availableParts;
        /** True only when the engine adapter suppressed the original or proved its absence. */
        public boolean replaceBody,replaceCape,visible,slim;
        public boolean nativeCapeAbsent;
        public Boolean chestEmpty;
        public com.google.gson.JsonObject nativeGlobal;
        /** Complete copied model observation for history writers, independent
         * of skin visibility. Invisibility may retain an actual draw's data;
         * missing roots/parts and unknown culling remain gaps. This does not
         * authorize rendering, native suppression or export readiness. */
        boolean modelObserved() {
            return "native-model-draw".equals(modelSource)&&Integer.valueOf(63).equals(availableParts)
                &&(visible||layerVisibility!=null&&layerVisibility.invisibleObserved());
        }
    }
    public static final class Movement {
        public boolean forward,backward,left,right,jump,sneak;
        public float[] analog;
        void validate() {
            finite(analog,2,1.001f);
        }
        boolean active() {
            return forward||backward||left||right||jump||sneak||analog[0]!=0||analog[1]!=0;
        }
    }
    /** Owned native container state. A missing group is unknown, while a
     * canonical empty slot is explicit; these meanings are never conflated. */
    public static final class Equipment {
        public ItemSlot[] armor,hands;
        void validate() {
            slots(armor,4);slots(hands,2);
        }
        private static void slots(ItemSlot[] slots,int size) {
            if(slots==null)return;
            if(slots.length!=size)throw new IllegalArgumentException("invalid equipment container size");
            for(ItemSlot slot:slots) {
                if(slot==null)throw new IllegalArgumentException("missing equipment slot");
                slot.validate();
            }
        }
        boolean attachmentsEmpty() {
            if(armor==null||hands==null)return false;
            for(ItemSlot slot:armor)if(!slot.empty)return false;
            for(ItemSlot slot:hands)if(!slot.empty)return false;
            return true;
        }
        boolean matches(Equipment actual) {
            return (armor==null||actual!=null&&same(armor,actual.armor))
                &&(hands==null||actual!=null&&same(hands,actual.hands));
        }
        private static boolean same(ItemSlot[] expected,ItemSlot[] actual) {
            if(actual==null||expected.length!=actual.length)return false;
            for(int i=0;i<expected.length;i++) {
                ItemSlot a=expected[i],b=actual[i];
                if(a.empty!=b.empty||a.item!=null&&!a.item.equals(b.item)||a.count!=null&&!a.count.equals(b.count)
                    ||a.iconKey!=null&&!a.iconKey.equals(b.iconKey)||a.iconFrame!=null&&!a.iconFrame.equals(b.iconFrame)
                    ||a.enchanted!=null&&!a.enchanted.equals(b.enchanted)
                    ||a.enchantmentListSize!=null&&!a.enchantmentListSize.equals(b.enchantmentListSize)
                    ||a.enchantmentListElementType!=null&&!a.enchantmentListElementType.equals(b.enchantmentListElementType)
                    ||a.enchantmentEntries!=null&&!sameEntries(a.enchantmentEntries,b.enchantmentEntries)
                    ||a.javaEnchantments!=null&&!a.javaEnchantments.same(b.javaEnchantments)
                    ||a.javaFoilInputs!=null&&!a.javaFoilInputs.matches(b.javaFoilInputs)
                    ||a.javaCapeLayers!=null&&!a.javaCapeLayers.matches(b.javaCapeLayers)
                    ||a.javaFoil!=null&&!a.javaFoil.equals(b.javaFoil)
                    ||a.forcedGlint!=null&&!a.forcedGlint.equals(b.forcedGlint)
                    ||a.nativeGlint!=null&&!a.nativeGlint.equals(b.nativeGlint))return false;
            }
            return true;
        }
        private static boolean sameEntries(EnchantmentEntry[] expected,EnchantmentEntry[] actual) {
            if(actual==null||expected.length!=actual.length)return false;
            for(int i=0;i<expected.length;i++)if(!Objects.equals(expected[i].id,actual[i].id)||!Objects.equals(expected[i].level,actual[i].level))return false;
            return true;
        }
    }
    /** Raw native short fields. Null is a checked missing/non-short tag, not
     * an unavailable list copy. No registry mapping or level normalization. */
    public static final class EnchantmentEntry {
        public Integer id,level;
        private void validate() {
            if(id!=null&&(id<Short.MIN_VALUE||id>Short.MAX_VALUE)||level!=null&&(level<Short.MIN_VALUE||level>Short.MAX_VALUE))
                throw new IllegalArgumentException("native enchantment field exceeds a signed short");
        }
    }
    public static final class ItemSlot {
        public boolean empty;
        public String item;
        /** Raw observed Item name; not sufficient to resolve an icon. */
        public String iconKey;
        /** Constrained native base-selector receipt; null for unresolved paths. */
        public Integer iconFrame;
        public Integer count;
        /** Native has-enchantment-list predicate; forced glint is separate. */
        public Boolean enchanted;
        /** Optional native ListTag header, not Java enchantment components. */
        public Integer enchantmentListSize,enchantmentListElementType;
        /** Original list order, at most 64 raw entries. Null means unavailable. */
        public EnchantmentEntry[] enchantmentEntries;
        /** Optional derived vanilla component projection, never a hasFoil or
         * native rendering receipt. Checked against raw observations on load. */
        public JavaEnchantmentComponents.Data javaEnchantments;
        transient String javaEnchantmentError;
        /** Explicit adapter facts; native forcedGlint is not substituted. */
        public JavaItemFoil.Input javaFoilInputs;
        /** Selected Java equipment-asset layer facts, independent of item ID. */
        public JavaCapeLayer.Input javaCapeLayers;
        /** Recomputed nullable Java foil selection, not a rendered-pass receipt. */
        public Boolean javaFoil;
        transient String javaFoilError;
        /** Checked base Item::isGlint inputs/result; overrides remain unknown. */
        public Boolean forcedGlint,nativeGlint;
        void validate() {
            if(count!=null&&(count<0||count>255)
                    ||item!=null&&(item.length()>193||!item.matches("[a-z0-9_.-]{1,64}:[a-z0-9_./-]{1,128}"))
                    ||empty&&(item!=null||!Integer.valueOf(0).equals(count)||Boolean.TRUE.equals(enchanted)
                        ||Boolean.TRUE.equals(forcedGlint)||Boolean.TRUE.equals(nativeGlint))
                    ||iconKey!=null&&(empty||item==null||!iconKey.matches("[A-Za-z0-9_.:/-]{1,193}"))
                    ||iconFrame!=null&&(empty||item==null||iconKey==null||iconFrame<0||iconFrame>65535)
                    ||!empty&&(Integer.valueOf(0).equals(count)||item!=null&&count==null))
                throw new IllegalArgumentException("invalid equipment item identity/count/empty state");
            if((enchantmentListSize==null)!=(enchantmentListElementType==null)
                    ||enchantmentListSize!=null&&(empty||!Boolean.TRUE.equals(enchanted)||enchantmentListSize<0||enchantmentListSize>256
                        ||enchantmentListElementType<0||enchantmentListElementType>11||enchantmentListSize>0&&enchantmentListElementType==0))
                throw new IllegalArgumentException("invalid native enchantment-list header");
            if(enchantmentEntries!=null) {
                if(empty||!Boolean.TRUE.equals(enchanted)||enchantmentListSize==null||enchantmentEntries.length!=enchantmentListSize
                        ||enchantmentEntries.length>64||enchantmentEntries.length!=0&&enchantmentListElementType!=10)
                    throw new IllegalArgumentException("native enchantment entries disagree with their list header");
                for(EnchantmentEntry entry:enchantmentEntries) {
                    if(entry==null)throw new IllegalArgumentException("missing native enchantment entry");
                    entry.validate();
                }
            }
            if(!empty&&nativeGlint!=null) {
                Boolean expected=forcedGlint==null?null:forcedGlint?Boolean.TRUE:enchanted;
                if(!nativeGlint.equals(expected))throw new IllegalArgumentException("native glint result disagrees with its observed inputs");
            }
            JavaEnchantmentComponents.Projection projected=JavaEnchantmentComponents.project(this);
            if(javaEnchantments!=null) {
                javaEnchantments.validate();
                if(!javaEnchantments.same(projected.data))throw new IllegalArgumentException("Java enchantment projection differs from its raw native source");
            }
            javaEnchantments=projected.data;javaEnchantmentError=projected.reason;
            if(javaFoilInputs!=null) {
                if(empty||item==null||count==null)throw new IllegalArgumentException("Java foil inputs need a copied nonempty item identity");
                javaFoilInputs.validate(item);javaFoilInputs=javaFoilInputs.copy();
            }
            if(javaCapeLayers!=null) {
                if(empty||item==null||count==null)throw new IllegalArgumentException("Java cape layers need a copied nonempty item identity");
                javaCapeLayers.validate();javaCapeLayers=javaCapeLayers.copy();
            }
            JavaItemFoil.Selection selected=JavaItemFoil.select(this);
            if(javaFoil!=null&&!javaFoil.equals(selected.value))throw new IllegalArgumentException("Java foil result differs from its explicit input branches");
            javaFoil=selected.value;javaFoilError=selected.reason;
        }
    }
    public static final class DrawColor {
        public String geometry,nativeCape;public long animationFrame;public float[] rgba;
        public Boolean sourceDithering,ditheringEnabled;
        public ActorShaderInputs shaderInputs;
        void validate() {
            if(geometry==null||geometry.isEmpty()||geometry.getBytes(java.nio.charset.StandardCharsets.UTF_8).length>128
                    ||animationFrame<0||rgba==null||nativeCape!=null&&!nativeCape.matches("engine:cape/[0-9a-f]{64}"))
                throw new IllegalArgumentException("invalid native layer color");
            for(int i=0;i<geometry.length();i++)if(Character.isISOControl(geometry.charAt(i)))throw new IllegalArgumentException("invalid native layer geometry");
            color(rgba);
            if(shaderInputs!=null)shaderInputs.validate();
        }
        void validate(Player player,boolean wing) {
            validate();String field=wing?"elytraColor":"capeColor";
            if(!"native-model-draw".equals(player.modelSource)||!Integer.valueOf(63).equals(player.availableParts)
                    ||player.nativeGlobal==null||!player.nativeGlobal.has(field)||!player.nativeGlobal.get(field).isJsonObject()
                    ||player.nativeGlobal.get("animationFrame").getAsLong()!=animationFrame
                    ||player.nativeGlobal.get(field+"Conflict").getAsBoolean()
                    ||wing&&(nativeCape!=null||!"geometry.elytra".equals(geometry)||player.wingMatrices==null||!Boolean.TRUE.equals(player.elytraEquipped))
                    ||!wing&&(nativeCape==null||!nativeCape.equals(player.nativeCape)||player.nativeCapeAbsent))
                throw new IllegalArgumentException("native layer color needs its owned draw");
            com.google.gson.JsonObject source=player.nativeGlobal.getAsJsonObject(field);
            if(!geometry.equals(source.get("geometry").getAsString())||animationFrame!=source.get("animationFrame").getAsLong()
                    ||!Objects.equals(nativeCape,source.get("nativeCape").isJsonNull()?null:source.get("nativeCape").getAsString())
                    ||source.getAsJsonArray("rgba").size()!=4)throw new IllegalArgumentException("native layer color identity differs");
            for(int i=0;i<4;i++)if(Float.floatToIntBits(rgba[i])!=Float.floatToIntBits(source.getAsJsonArray("rgba").get(i).getAsFloat()))
                throw new IllegalArgumentException("native layer color channels differ");
            if(!Objects.equals(sourceDithering,nullableBoolean(source,"sourceDithering"))
                    ||!Objects.equals(ditheringEnabled,nullableBoolean(source,"ditheringEnabled")))
                throw new IllegalArgumentException("native layer dithering differs");
            if(!ActorShaderInputs.same(shaderInputs,ActorShaderInputs.owned(source.get("shaderInputs"))))
                throw new IllegalArgumentException("native layer shader inputs differ");
        }
    }
    public static final class CapeReplayColor {
        public String geometry,nativeCape;public long animationFrame;
        public int slot=AttachmentSample.CAPE_SLOT;
        public float[] sourceRgba,rgba;public AttachmentSample.ReplayReceipt replay;
        public Boolean sourceDithering,ditheringEnabled;
        public ActorShaderInputs shaderInputs;
        public ActorShaderInputs sourceShaderInputs;
        public Boolean shaderInputsApplied;
        public int shaderInputDraws,shaderInputCopies;
        void validate(Player player) {
            validate(player,false);
        }
        void validate(Player player,boolean wing) {
            DrawColor original=wing?player.nativeWingDrawColor:player.nativeCapeDrawColor;
            String field=wing?"elytraReplayColor":"capeReplayColor";
            if(original==null||sourceRgba==null||rgba==null||player.replaceElytra||replay==null
                    ||slot!=(wing?AttachmentSample.WING_SLOT:AttachmentSample.CAPE_SLOT)
                    ||wing&&(nativeCape!=null||!"geometry.elytra".equals(geometry)||!Boolean.TRUE.equals(player.elytraEquipped))
                    ||!wing&&(player.nativeCapeSuppressed||player.nativeCapeHidden||player.replaceCape||player.nativeCapeAbsent||!Objects.equals(nativeCape,player.nativeCape)))
                throw new IllegalArgumentException("replayed layer color needs its unsuppressed native draw");
            original.validate(player,wing);color(sourceRgba);color(rgba);replay.validate();
            if(shaderInputs!=null)shaderInputs.validate();
            if(sourceShaderInputs!=null)sourceShaderInputs.validate();
            ActorShaderInputs sourceInputs=sourceShaderInputs==null?shaderInputs:sourceShaderInputs;
            if(shaderInputsApplied!=null&&(shaderInputDraws<0||shaderInputDraws>4096||shaderInputCopies<0||shaderInputCopies>shaderInputDraws
                    ||shaderInputsApplied!=(shaderInputDraws>0&&shaderInputDraws==shaderInputCopies)))
                throw new IllegalArgumentException("invalid native cape material copy counts");
            // DrawColor describes the unmodified constructor output. The
            // receipt's applied flag may differ, like its applied RGBA.
            if(!Objects.equals(geometry,original.geometry)||animationFrame!=original.animationFrame||!Arrays.equals(sourceRgba,original.rgba)
                    ||!Objects.equals(sourceDithering,original.sourceDithering)||!Objects.equals(sourceDithering,original.ditheringEnabled)
                    ||!ActorShaderInputs.same(sourceInputs,original.shaderInputs)
                    ||player.nativeGlobal.get(field+"Conflict").getAsBoolean())
                throw new IllegalArgumentException("replayed cape color source differs");
            com.google.gson.JsonObject owned=player.nativeGlobal.getAsJsonObject(field);
            if(owned==null||!geometry.equals(owned.get("geometry").getAsString())
                    ||!Objects.equals(nativeCape,owned.get("nativeCape").isJsonNull()?null:owned.get("nativeCape").getAsString())
                    ||owned.has("slot")&&owned.get("slot").getAsInt()!=slot||wing&&!owned.has("slot")
                    ||animationFrame!=owned.get("animationFrame").getAsLong())throw new IllegalArgumentException("replayed cape color receipt differs");
            channels(owned,"sourceRgba",sourceRgba);channels(owned,"rgba",rgba);
            if(!Objects.equals(sourceDithering,nullableBoolean(owned,"sourceDithering"))
                    ||!Objects.equals(ditheringEnabled,nullableBoolean(owned,"ditheringEnabled")))
                throw new IllegalArgumentException("replayed cape dithering receipt differs");
            if(!ActorShaderInputs.same(shaderInputs,ActorShaderInputs.owned(owned.get("shaderInputs"))))
                throw new IllegalArgumentException("replayed cape shader-input receipt differs");
            ActorShaderInputs ownedSource=ActorShaderInputs.owned(owned.get("sourceShaderInputs"));
            if(ownedSource==null)ownedSource=ActorShaderInputs.owned(owned.get("shaderInputs"));
            if(!ActorShaderInputs.same(sourceInputs,ownedSource)
                    ||!Objects.equals(shaderInputsApplied,nullableBoolean(owned,"shaderInputsApplied"))
                    ||shaderInputsApplied!=null&&(!owned.has("shaderInputDraws")||!owned.has("shaderInputCopies")
                        ||shaderInputDraws!=owned.get("shaderInputDraws").getAsInt()||shaderInputCopies!=owned.get("shaderInputCopies").getAsInt()))
                throw new IllegalArgumentException("replayed cape material-copy receipt differs");
            com.google.gson.JsonObject identity=owned.getAsJsonObject("replay");
            if(!replay.manifestHash.equals(identity.get("manifestHash").getAsString())||replay.timeMs!=identity.get("timeMs").getAsLong()
                    ||replay.timeRemainder!=identity.get("timeRemainder").getAsInt()||replay.timeDenominator!=identity.get("timeDenominator").getAsInt())
                throw new IllegalArgumentException("replayed cape color clock receipt differs");
        }
        private static void channels(com.google.gson.JsonObject owned,String key,float[] expected) {
            com.google.gson.JsonArray values=owned.getAsJsonArray(key);
            if(values==null||values.size()!=4)throw new IllegalArgumentException("missing replayed cape channels");
            for(int i=0;i<4;i++)if(Float.floatToIntBits(expected[i])!=Float.floatToIntBits(values.get(i).getAsFloat()))
                throw new IllegalArgumentException("replayed cape channels differ");
        }
    }
    public static final class EmotePose {
        public String token;
        public long animationFrame,timeMs;
        public Double sampleTimeMs;
        public float seconds;
        public float[][] referenceParts,parts;
        void validate(Player player) {
            if(token==null||!token.matches("[0-9a-f]{64}")||animationFrame<0||timeMs<0||timeMs>1_000_000_000_000L
                    ||!Float.isFinite(seconds)||seconds<0||seconds>86400||referenceParts==null||parts==null
                    ||sampleTimeMs!=null&&(!Double.isFinite(sampleTimeMs)||sampleTimeMs<timeMs||sampleTimeMs>=timeMs+1d||sampleTimeMs>1_000_000_000_000L)
                    ||!"native-model-draw".equals(player.modelSource)||!Integer.valueOf(63).equals(player.availableParts)
                    ||player.partMatrices==null||!Arrays.deepEquals(referenceParts,player.partMatrices)
                    ||player.nativeGlobal==null||!player.nativeGlobal.has("animationFrame")
                    ||player.nativeGlobal.get("animationFrame").getAsLong()!=animationFrame)throw new IllegalArgumentException("invalid native emote target");
            matrices(referenceParts);matrices(parts);
        }
    }
    /** Observed pack instances, each containing selected-subpack, overlay and
     * base roots in engine lookup order. An error never carries partial roots. */
    public static final class ResourceStack {
        public String revision,error;
        public String[][] packs;
        void validate() {
            if(revision==null||!revision.matches("[0-9a-f]{64}")||packs==null||packs.length>64
                    ||error!=null&&(error.isEmpty()||error.length()>512||packs.length!=0)
                    ||error==null&&packs.length==0)throw new IllegalArgumentException("invalid native resource stack");
            int count=0,total=0;
            for(String[] pack:packs) {
                if(pack==null||pack.length==0)throw new IllegalArgumentException("native resource pack has no roots");
                for(String root:pack) {
                    if(++count>64||root==null||root.isEmpty()||root.length()>4096||!new java.io.File(root).isAbsolute())throw new IllegalArgumentException("invalid native resource root");
                    total+=root.length();if(total>64*1024)throw new IllegalArgumentException("native resource roots exceed their budget");
                    for(int i=0;i<root.length();i++)if(Character.isISOControl(root.charAt(i)))throw new IllegalArgumentException("control character in native resource root");
                }
            }
        }
    }
    public static WorldFrame parse(String json,float[] depth) {
        return parse(json,depth,false);
    }
    /** Same actor/appearance/camera validation, without a CPU terrain image.
     * Native geometry uses the actual bound DSV; this object cannot become a
     * software-renderable WorldState or diagnostic depth snapshot. */
    static WorldFrame parseGeometry(String json) {return parse(json,null,true);}
    boolean geometryOnly(){return geometryOnly;}
    JavaEntityShader entityShader(){return entityShader;}
    private static WorldFrame parse(String json,float[] depth,boolean geometryOnly) {
        if(json==null || json.length()>1024*1024) throw new IllegalArgumentException("world snapshot exceeds 1 MiB");
        WorldFrame f=GSON.fromJson(json,WorldFrame.class);
        if(f==null || f.schema!=1 || f.width<1 || f.width>3840 || f.height<1 || f.height>2160
                || f.session==null || !f.session.matches("[A-Za-z0-9_-]{1,64}") || f.sequence<0 || f.clockMs<0
                || f.players==null || f.players.length>128 || f.server!=null&&f.server.length()>253
                || f.dimension==null || f.dimension.length()>64) throw new IllegalArgumentException("invalid world snapshot");
        finite(f.viewProjection,16,1e8f);finite(f.camera,6,32_000_000);
        f.shaderClock=NativeShaderClock.observe(f);
        if(f.javaEntityShaderInputs!=null) {
            if(f.cameraRelative==null)throw new IllegalArgumentException("entity shader inputs need a declared position space");
            f.entityShader=new JavaEntityShader(f.javaEntityShaderInputs);
        }
        if(f.projectionContext!=null)f.equipmentProjection=f.projectionContext.equipment(f.viewProjection,f.cameraRelative);
        if(f.itemResources!=null)f.itemResources.validate();
        if(f.camera[5]<1 || f.camera[5]>179) throw new IllegalArgumentException("invalid camera FOV");
        f.geometryOnly=geometryOnly;
        if(!geometryOnly) {
            if(depth==null || depth.length!=f.width*f.height) throw new IllegalArgumentException("world rendering needs matching terrain depth");
            for(float d:depth) if(!Float.isFinite(d) || d<0 || d>1) throw new IllegalArgumentException("invalid terrain depth");
            f.depth=depth.clone();
        }
        Set<String> ids=new HashSet<String>();
        for(Player p:f.players) {
            if(p==null || p.id==null || p.id.length()!=36 || !UUID.fromString(p.id).toString().equalsIgnoreCase(p.id)
                    || !ids.add(p.id.toLowerCase(Locale.ROOT)) || p.name==null || p.name.length()>64
                    || p.skin!=null&&!p.skin.matches("(?:engine|look):[A-Za-z0-9_/-]{1,120}")
                    || p.cape!=null&&!p.cape.matches("(?:engine|look):[A-Za-z0-9_/-]{1,120}")
                    || p.nativeSkin!=null&&!p.nativeSkin.matches("engine:skin/[0-9a-f]{64}")
                    || p.nativeCape!=null&&!p.nativeCape.matches("engine:cape/[0-9a-f]{64}")
                    || p.chestItem!=null&&(p.chestItem.length()>193||!p.chestItem.matches("[a-z0-9_.-]{1,64}:[a-z0-9_./-]{1,128}")||!Boolean.FALSE.equals(p.chestEmpty))
                    || p.capeHash!=null&&!p.capeHash.matches("[0-9a-f]{40}")
                    || p.nativeCapeSuppressed&&(!p.replaceCape || p.nativeCape==null || p.capeHash==null || p.cape==null
                        || p.nativeCapeAbsent || p.capeMotion==null || !Boolean.TRUE.equals(p.chestEmpty)
                        || p.partMatrices==null || !Integer.valueOf(63).equals(p.availableParts)
                        || p.nativeCapeHidden || !p.cape.matches("look:"+p.capeHash+"(?:/[0-9]{1,4}|/native)?"))
                    || p.nativeCapeHidden&&(p.nativeCape==null||p.nativeCapeAbsent||p.nativeCapeSuppressed||p.replaceCape
                        ||p.replaceElytra||p.cape!=null||p.capeHash!=null||!Boolean.TRUE.equals(p.chestEmpty)
                        ||p.partMatrices==null||!Integer.valueOf(63).equals(p.availableParts)||!"native-model-draw".equals(p.modelSource))
                    || p.replaceBody&&p.skin==null) throw new IllegalArgumentException("invalid world player");
            finite(p.position,5,32_000_000);finite(p.model,12,32_000_000f);
            parts(p.partPivots,256);parts(p.partRotations,64);
            matrices(p.partMatrices);
            p.skinMaterial=JavaSkinMaterial.parse(p.javaSkinMaterial);
            p.skinVisibility=JavaSkinVisibility.parse(p.javaSkinVisibility);
            p.layerVisibility=JavaLayerVisibility.parse(p.javaLayerVisibility);
            p.wingGlint=GlintSample.parse(p.javaWingGlint);
            if(p.wingGlint!=null&&p.wingGlint.selected&&(f.cameraRelative==null||f.equipmentProjection==null))
                throw new IllegalArgumentException("wing glint needs a declared layered camera");
            if(p.skinMaterial!=null&&f.cameraRelative==null)throw new IllegalArgumentException("skin material needs a declared position space");
            if(p.nativeSkinColor!=null) {
                color(p.nativeSkinColor);
                if(!"native-model-draw".equals(p.modelSource)||!Integer.valueOf(63).equals(p.availableParts)
                        ||p.nativeGlobal==null||!p.nativeGlobal.has("skinColor")||!p.nativeGlobal.get("skinColor").isJsonArray()
                        ||p.nativeGlobal.getAsJsonArray("skinColor").size()!=4
                        ||p.nativeGlobal.has("skinColorConflict")&&p.nativeGlobal.get("skinColorConflict").getAsBoolean())
                    throw new IllegalArgumentException("native skin color needs an owned skin draw");
                for(int i=0;i<4;i++)if(Float.floatToIntBits(p.nativeSkinColor[i])!=Float.floatToIntBits(p.nativeGlobal.getAsJsonArray("skinColor").get(i).getAsFloat()))
                    throw new IllegalArgumentException("native skin color differs from its draw");
            }
            if(p.emotePose!=null)p.emotePose.validate(p);
            if(p.attachmentPoses!=null)p.attachmentPoses.validate(p);
            wings(p.wingMatrices);
            if(p.wingMatrices!=null&&(!"minecraft:elytra".equals(p.chestItem)
                    ||!"native-model-draw".equals(p.modelSource)))
                throw new IllegalArgumentException("wing matrices need an owned equipped-item draw");
            if(p.replaceElytra&&(p.wingMatrices==null||p.partMatrices==null
                    ||!Integer.valueOf(63).equals(p.availableParts)||p.wingHash==null
                    ||!p.wingHash.matches("[0-9a-f]{40}")||p.wingTexture==null
                    ||!p.wingTexture.matches("look:"+p.wingHash+"(?:/[0-9]{1,4})?")||p.replaceCape)
                    ||!p.replaceElytra&&(p.wingTexture!=null||p.wingHash!=null))
                throw new IllegalArgumentException("invalid native wing replacement receipt");
            if(p.nativeCapeDrawColor!=null)p.nativeCapeDrawColor.validate(p,false);
            if(p.nativeCapeReplayColor!=null)p.nativeCapeReplayColor.validate(p);
            if(p.nativeWingDrawColor!=null)p.nativeWingDrawColor.validate(p,true);
            if(p.nativeWingReplayColor!=null)p.nativeWingReplayColor.validate(p,true);
            if(p.availableParts!=null && (p.availableParts<0 || p.availableParts>63
                    || p.replaceBody&&p.availableParts!=63 || p.replaceCape&&(p.availableParts&2)==0))
                throw new IllegalArgumentException("replacement needs complete attachment poses");
            if(p.capeMotion!=null)p.capeMotion.validate();
            if(p.capeState!=null) {
                p.capeState.validate();
                if(f.nativeTiming==null||!f.nativeTiming.has("ticks")||f.nativeTiming.get("ticks").getAsLong()!=p.capeState.tick)
                    throw new IllegalArgumentException("cape filter belongs to a different native tick");
                if(p.capeState.wingState!=null) {
                    com.google.gson.JsonElement partial=f.nativeTiming.get("partial");
                    if(partial==null||!partial.isJsonPrimitive()||!partial.getAsJsonPrimitive().isNumber())
                        throw new IllegalArgumentException("Java wing filter needs a native tick fraction");
                    float phase=partial.getAsFloat();
                    if(!Float.isFinite(phase)||phase<0||phase>1)throw new IllegalArgumentException("invalid native wing fraction");
                }
            }
            Boolean equipped=Boolean.TRUE.equals(p.chestEmpty)?Boolean.FALSE:p.chestItem==null?null:Boolean.valueOf("minecraft:elytra".equals(p.chestItem));
            if(p.elytraEquipped!=null&&!p.elytraEquipped.equals(equipped))throw new IllegalArgumentException("inconsistent native elytra equipment");
            if(p.movement!=null)p.movement.validate();
            if(p.equipment!=null) {
                p.equipment.validate();
                if(p.equipment.armor!=null) {
                    ItemSlot chest=p.equipment.armor[1];
                    if(p.chestEmpty==null||chest.empty!=p.chestEmpty||!Objects.equals(chest.item,p.chestItem))
                        throw new IllegalArgumentException("equipment chest disagrees with native chest observation");
                    if(p.wingGlint!=null&&p.wingGlint.selected&&(chest.empty||Boolean.FALSE.equals(chest.javaFoil)))
                        throw new IllegalArgumentException("selected wing glint conflicts with the copied Java foil selection");
                }
            }
            p.id=p.id.toLowerCase(Locale.ROOT);
        }
        if(f.localPlayer!=null) f.localPlayer=f.localPlayer.toLowerCase(Locale.ROOT);
        if(f.localPlayer!=null&&!ids.contains(f.localPlayer)) throw new IllegalArgumentException("local player is absent from the snapshot");
        f.replayCosmeticSample=CosmeticClock.replaySample(f);
        return f;
    }
    public Player local() {
        if(localPlayer!=null) for(Player p:players) if(p.id.equals(localPlayer)) return p;
        return null;
    }
    /** Owned input depth for opt-in diagnostic comparisons. */
    float[] depthSnapshot(){
        if(geometryOnly||depth==null)throw new IllegalStateException("geometry input has no terrain depth image");
        return depth.clone();
    }
    /** Snapshot identity alone cannot attest that the native draw accepted its
     * replacement. Export must wait through older buffered body/cape plans. */
    boolean recordedAppearanceReady(BedrockPlatform platform) {
        if(!platform.replayAppearanceActive())return false;
        JavaEntityShader recordedShader=platform.recordedEntityShader(this);
        Looks looks=com.arcticlauncher.client.ArcticClient.looks();
        for(Player p:players) {
            if(!p.visible)continue;
            AvatarRenderer.Avatar desired=platform.avatar(UUID.fromString(p.id),cosmeticTimeMillis());
            if(!platform.recordedEquipmentMatches(UUID.fromString(p.id),p.equipment))return false;
            if(!platform.recordedSkinReady(UUID.fromString(p.id)))return false;
            if(!platform.recordedCapeReady(UUID.fromString(p.id)))return false;
            boolean body=desired.skin!=null||desired.emote!=null;
            if(body||desired.cape!=null||!desired.worn.isEmpty()) {
                if(recordedShader==null)return false;
                BodySample sample=platform.recordedBodyPose(UUID.fromString(p.id));
                if(sample==null||!sample.hasColor()||!sample.hasPackedLight()||!sample.hasLayerVisibility())return false;
                if(body&&(!sample.hasSkinMaterial()||!sample.hasSkinVisibility()))return false;
            }
            if((body||desired.cape!=null||!desired.worn.isEmpty())&&platform.recordedModelRoot(UUID.fromString(p.id))==null)return false;
            // Native armor/held meshes do not yet consume the replacement
            // emote pose. Do not attest complete appearance while those meshes
            // still animate independently, or when occupancy is unknown.
            if(desired.emote!=null&&(p.equipment==null||!p.equipment.attachmentsEmpty()))return false;
            if(desired.emote!=null&&(p.emotePose==null||!NativeBridge.emotePoseMatches(p.emotePose,this,p.id,desired.emote)))return false;
            if(desired.emote==null&&p.emotePose!=null)return false;
            if(body) {
                BedrockPlatform.BodyTexture plan=platform.bodyTexture(p,looks);
                if(plan==null||!p.replaceBody||!plan.key.equals(p.skin)||plan.slim!=p.slim
                        ||platform.texture(p.skin)==null||p.partMatrices==null||!Integer.valueOf(63).equals(p.availableParts))return false;
            } else if(p.replaceBody)return false;
            if(!desired.worn.isEmpty()&&(p.partMatrices==null||!Integer.valueOf(63).equals(p.availableParts)))return false;
            if(desired.cape!=null) {
                if(!platform.recordedCapeColorReady(UUID.fromString(p.id)))return false;
                // Sampled wing curves are available for playback, but native
                // replacement, interpolation fidelity and lighting/glint are
                // unverified. Keep wing export gated until those agree.
                if(p.elytra||Boolean.TRUE.equals(p.elytraEquipped)||!Boolean.TRUE.equals(p.chestEmpty)
                        ||p.partMatrices==null||!Integer.valueOf(63).equals(p.availableParts))return false;
                if(platform.nativeCapeReplayRequested(p)) {
                    // Actual current native palette copies must acknowledge the
                    // archive and exact rational sample; Java physics is unused.
                    if(!platform.nativeCapeReplayReady(p))return false;
                } else {
                    if(!p.replaceCape||!platform.recordedCapeMotionActive()
                            ||platform.capeMotion(UUID.fromString(p.id),p.capeMotion)==null)return false;
                    if(p.nativeCapeSuppressed) {
                        BedrockPlatform.CapeTexture plan=platform.capeTexture(p,cosmeticTimeMillis(),looks);
                        if(plan==null||!plan.hash.equals(p.capeHash)||platform.texture(p.cape)==null
                                ||plan.key.endsWith("/native")!=p.cape.endsWith("/native"))return false;
                    } else if(!p.nativeCapeAbsent||p.cape!=null)return false;
                }
            } else {
                if(p.nativeCapeSuppressed||p.replaceCape&&p.cape!=null||p.replaceElytra)return false;
                if(platform.recordedCapeAbsent(UUID.fromString(p.id))&&!p.nativeCapeHidden
                        &&(!p.nativeCapeAbsent||p.nativeCape!=null||p.cape!=null))return false;
            }
        }
        return true;
    }
    public BufferedImage render(BedrockPlatform platform) {
        if(geometryOnly||depth==null)throw new IllegalStateException("geometry input cannot produce a software world image");
        float[] sortOrigin=cameraRelative==null?null:cameraRelative?new float[3]
            :new float[]{(float)camera[0],(float)camera[1],(float)camera[2]};
        Raster3D raster=new Raster3D(width,height,viewProjection,depth,sortOrigin,
            JavaEntityLighting.world(dimension,cameraRelative),equipmentProjection,
            platform.replayAppearanceActive()?platform.recordedEntityShader(this):entityShader,platform.javaReferenceAssets,
            this,platform.softwareAvatarSimplePhases);
        emitAvatars(platform,(player,avatar,model)->AvatarRenderer.emit(avatar,raster,model));
        return raster.finish();
    }
    /** Same acknowledged appearances/poses as render(), before projection.
     * The packet owns its data and does not authorize native suppression. */
    byte[] geometry(BedrockPlatform platform) {
        NativeAvatarGeometry.Builder builder=new NativeAvatarGeometry.Builder();
        emitAvatars(platform,(player,avatar,model)-> {
            builder.actor(player.id);
            AvatarRenderer.emit(avatar,builder,model);
        });
        JavaEntityShader material=platform.replayAppearanceActive()?platform.recordedEntityShader(this):entityShader;
        NativeAvatarGeometry geometry=builder.finish();
        JavaSimplePhases.Result phases=platform.avatarSimplePhases.collect(this,geometry);
        return NativeGeometryPacket.encode(this,equipmentProjection,geometry,material,phases);
    }
    private interface AvatarConsumer {
        void emit(Player player,AvatarRenderer.Avatar avatar,float[] model);
    }
    private void emitAvatars(BedrockPlatform platform,AvatarConsumer target) {
        for(Player p:players) {
            if(!p.visible) continue;
            UUID id=UUID.fromString(p.id);
            // Like Java's cosmetics layer, keep the visible actor subscribed
            // even before its first remote emote starts or finishes loading.
            if(!platform.replayAppearanceActive()&&com.arcticlauncher.client.ArcticClient.looks()!=null)
                com.arcticlauncher.client.ArcticClient.looks().cosmetics().watch(id);
            AvatarRenderer.Avatar avatar=platform.avatar(id,cosmeticTimeMillis());
            if(!platform.replayAppearanceActive()) {
                avatar.javaCapeLayer=JavaCapeLayer.select(p.equipment);
                if(avatar.javaCapeLayer==JavaCapeLayer.Selection.UNKNOWN&&Boolean.TRUE.equals(p.chestEmpty))
                    avatar.javaCapeLayer=JavaCapeLayer.Selection.PLAIN;
            }
            if(avatar.skin==null)avatar.slim=p.slim;
            avatar.partPivots=p.partPivots;avatar.partRotations=p.partRotations;
            avatar.partMatrices=p.partMatrices;
            avatar.packedLight=p.packedLight;
            avatar.skinColor=p.nativeSkinColor;
            avatar.skinMaterial=p.skinMaterial;
            avatar.skinVisibility=p.skinVisibility;
            avatar.layerVisibility=p.layerVisibility;
            avatar.capeColor=p.nativeCapeDrawColor==null?null:p.nativeCapeDrawColor.rgba;
            float[] wingColor=p.nativeWingDrawColor==null?null:p.nativeWingDrawColor.rgba;
            float[] model=p.model;
            if(platform.replayAppearanceActive()) {
                BodySample recorded=platform.recordedBodyPose(id);
                // Never borrow the viewer's live light. Old archives, gaps
                // and changing-light intervals leave provenance unknown.
                avatar.packedLight=recorded==null?null:recorded.packedLight();
                avatar.skinMaterial=recorded==null?null:recorded.skinMaterial();
                avatar.skinVisibility=recorded==null?null:recorded.skinVisibility();
                avatar.layerVisibility=recorded==null?null:recorded.layerVisibility();
                if(recorded!=null){avatar.partMatrices=recorded.copyParts();avatar.skinColor=recorded.copyColor();}
                else avatar.skinColor=null;
                LayerColorSample colors=platform.recordedLayerColors(id);
                avatar.capeColor=colors==null?null:colors.color(LayerColorSample.CAPE);
                wingColor=colors==null?null:colors.color(LayerColorSample.WING);
                float[] root=platform.recordedModelRoot(id);if(root!=null)model=root;
                // A buffered suppressed draw still needs its acknowledged body
                // during a gap. Missing recorded data cannot pass capture's
                // readiness check or authorize a new body plan for this track.
            }
            avatar.availableParts=p.availableParts==null?63:p.availableParts;
            avatar.opacity=avatar.skinColor==null?1:avatar.skinColor[3];
            avatar.capeMotion=avatar.cape!=null||p.replaceCape&&p.cape!=null?platform.capeMotion(id,p.capeMotion):p.capeMotion;
            avatar.elytra=p.elytra||Boolean.TRUE.equals(p.elytraEquipped);
            if(p.replaceBody) {
                // This is the texture/arm model whose readiness authorized the
                // native draw's suppression. A newly downloaded look applies
                // only after a subsequent native draw acknowledges that plan.
                avatar.skinTexture(platform.texture(p.skin),platform.textureId(p.skin));avatar.slim=p.slim;
            } else { avatar.skin=null;avatar.emote=null; }
            if(p.replaceBody&&EmotePrograms.observedAt(p.emotePose,cosmeticTimeExactMillis())) {
                // Draw the target actually carried by this buffered native
                // frame, even when a newer menu edit changed or stopped it.
                // A retained animation supplies matching elbow/knee bending.
                avatar.preposedParts=p.emotePose.parts;avatar.emoteSeconds=p.emotePose.seconds;
                avatar.emote=NativeBridge.acknowledgedEmote(p.emotePose,this,p.id);
            } else avatar.emote=null;
            if(p.replaceElytra) {
                // Replay cannot borrow the viewer's glint clock/options when
                // its exact/stable recorded observation is unavailable.
                avatar.wingGlint=platform.replayAppearanceActive()?platform.recordedWingGlint(id,this):p.wingGlint;
                avatar.capeColor=wingColor;
                // The native receipt owns suppression and its cape art. Replay
                // replaces the pose with its recorded curve only; a missing
                // curve never falls back to the viewer's live controller.
                String texture=p.wingTexture;
                String frame=platform.capeFrame(p.wingHash,cosmeticTimeMillis());
                if(frame!=null&&platform.texture(frame)!=null)texture=frame;
                avatar.capeTexture(platform.texture(texture),platform.textureId(texture));avatar.elytra=true;avatar.wingMatrices=p.wingMatrices;
                if(!platform.replayAppearanceActive()&&p.capeState!=null&&p.capeState.wingState!=null)
                    avatar.javaWingMotion=p.capeState.wingState.sample(nativeTiming.get("partial").getAsFloat());
                if(platform.replayAppearanceActive()) {
                    WingSample recorded=platform.recordedWingPose(id);
                    if(recorded==null) {avatar.cape=null;avatar.wingMatrices=null;}
                    else {avatar.wingMatrices=recorded.wings;avatar.wingReferenceBody=recorded.body;}
                    if(platform.recordedJavaWings()) {
                        avatar.javaWingMotion=platform.recordedJavaWingMotion(id);
                        // A missing tick in a v2 recording is an explicit gap;
                        // never replace it with this viewer's native animation.
                        if(avatar.javaWingMotion==null)avatar.cape=null;
                    }
                }
            } else if(p.nativeCapeHidden||!p.replaceCape) avatar.cape=null;
            else if(p.nativeCapeSuppressed) {
                // The native draw acknowledged this cape, which may differ
                // from a newer menu edit. Animate its loaded frames on this
                // world clock until a later draw accepts a different cape.
                String texture=p.cape;
                String frame=texture.endsWith("/native")?null:platform.capeFrame(p.capeHash,cosmeticTimeMillis());
                if(frame!=null&&platform.texture(frame)!=null)texture=frame;
                avatar.capeTexture(platform.texture(texture),platform.textureId(texture));
            } else if(avatar.cape==null && p.cape!=null) avatar.capeTexture(platform.texture(p.cape),platform.textureId(p.cape));
            if(!avatar.elytra&&platform.recordedCapeMotionActive()&&avatar.capeMotion==null)avatar.cape=null;
            target.emit(p,avatar,model);
        }
    }
    private static Boolean nullableBoolean(com.google.gson.JsonObject object,String key) {
        com.google.gson.JsonElement value=object.get(key);if(value==null||value.isJsonNull())return null;
        if(!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isBoolean())throw new IllegalArgumentException("invalid native boolean receipt");
        return value.getAsBoolean();
    }
    static void color(float[] values) {
        if(values==null)return;
        if(values.length!=4)throw new IllegalArgumentException("native color needs four channels");
        for(float value:values)if(!Float.isFinite(value)||value<0||value>1)throw new IllegalArgumentException("invalid native color channel");
    }
    private static void parts(float[][] values,float limit) {
        if(values==null) return;
        if(values.length!=6) throw new IllegalArgumentException("expected six player parts");
        for(float[] p:values) finite(p,3,limit);
    }
    private static void matrices(float[][] values) {
        if(values==null) return;
        if(values.length!=6) throw new IllegalArgumentException("expected six player matrices");
        for(float[] m:values)rigid(m);
    }
    static void rigid(float[] m) {
        finite(m,12,256);
        for(int a=0;a<3;a++) for(int b=0;b<3;b++) {
            float dot=0;for(int r=0;r<3;r++)dot+=m[r*4+a]*m[r*4+b];
            if(Math.abs(dot-(a==b?1:0))>.001f) throw new IllegalArgumentException("part matrix must be rigid");
        }
        float det=m[0]*(m[5]*m[10]-m[6]*m[9])-m[1]*(m[4]*m[10]-m[6]*m[8])+m[2]*(m[4]*m[9]-m[5]*m[8]);
        if(Math.abs(det-1)>.001f) throw new IllegalArgumentException("part matrix must preserve handedness");
    }
    /** Native wings include nonuniform scale, unlike the six rigid skin parts.
     * Reject reflections, degenerate axes, shear and unbounded transforms. */
    static void wings(float[][] values) {
        if(values==null)return;
        if(values.length!=2)throw new IllegalArgumentException("expected two wing matrices");
        for(float[] m:values) {
            finite(m,12,256);
            float[] lengths=new float[3];
            for(int c=0;c<3;c++) {
                for(int r=0;r<3;r++)lengths[c]+=m[r*4+c]*m[r*4+c];
                lengths[c]=(float)Math.sqrt(lengths[c]);
                if(lengths[c]<.05f||lengths[c]>8)throw new IllegalArgumentException("invalid wing scale");
            }
            for(int a=0;a<3;a++)for(int b=a+1;b<3;b++) {
                float dot=0;for(int r=0;r<3;r++)dot+=m[r*4+a]*m[r*4+b];
                if(Math.abs(dot)>.001f*lengths[a]*lengths[b])throw new IllegalArgumentException("wing matrix contains shear");
            }
            float det=m[0]*(m[5]*m[10]-m[6]*m[9])-m[1]*(m[4]*m[10]-m[6]*m[8])+m[2]*(m[4]*m[9]-m[5]*m[8]);
            if(Math.abs(det/(lengths[0]*lengths[1]*lengths[2])-1)>.001f)
                throw new IllegalArgumentException("wing matrix must preserve handedness");
        }
    }
    private static void finite(float[] values,int size,float limit) {
        if(values==null || values.length!=size) throw new IllegalArgumentException("invalid transform shape");
        for(float f:values) if(!Float.isFinite(f)||Math.abs(f)>limit) throw new IllegalArgumentException("invalid transform");
    }
    private static void finite(double[] values,int size,double limit) {
        if(values==null || values.length!=size) throw new IllegalArgumentException("invalid position shape");
        for(double f:values) if(!Double.isFinite(f)||Math.abs(f)>limit) throw new IllegalArgumentException("invalid position");
    }
}
