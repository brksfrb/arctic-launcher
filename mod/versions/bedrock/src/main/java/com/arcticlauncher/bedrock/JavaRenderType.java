package com.arcticlauncher.bedrock;

import java.util.EnumMap;
import java.util.concurrent.ConcurrentHashMap;

/** Reference identities from the supported Java RenderTypes factory calls.
 * These are owned renderer objects, never engine pointers or prepared-GPU
 * identities. A logical texture alone cannot choose a factory or a phase. */
public final class JavaRenderType {
    public enum Factory {
        ENTITY_TRANSLUCENT(1),ENTITY_TRANSLUCENT_EMISSIVE(2),ENTITY_SOLID(3),ARMOR_CUTOUT_NO_CULL(4);
        public final int wire;
        Factory(int wire){this.wire=wire;}
    }
    private static final int MAX_REFERENCES=65_536;
    private static final EnumMap<Factory,ConcurrentHashMap<String,JavaRenderType>> CACHE=
        new EnumMap<Factory,ConcurrentHashMap<String,JavaRenderType>>(Factory.class);
    private static long references;
    static {for(Factory factory:Factory.values())CACHE.put(factory,new ConcurrentHashMap<String,JavaRenderType>());}
    public final Factory factory;
    public final String texture;
    public final long reference;
    private JavaRenderType(Factory factory,String texture,long reference) {
        this.factory=factory;this.texture=texture;this.reference=reference;
    }
    /** The current emitter uses one-argument factories. Both translucent
     * wrappers select their Boolean.TRUE cache key on Java 26.2. Distinct
     * variants must get a distinct recipe rather than reusing this entry. */
    static JavaRenderType forPass(NativeAvatarGeometry.Pass pass,String texture) {
        if(texture==null)return null;
        JavaTextureId.validate(texture);
        Factory factory;
        switch(pass) {
            case COSMETIC_BASE:case SKIN_TRANSLUCENT:case BENT_SKIN:factory=Factory.ENTITY_TRANSLUCENT;break;
            case COSMETIC_GLOW:factory=Factory.ENTITY_TRANSLUCENT_EMISSIVE;break;
            case CAPE_SOLID:factory=Factory.ENTITY_SOLID;break;
            case WING_CUTOUT:factory=Factory.ARMOR_CUTOUT_NO_CULL;break;
            default:return null;
        }
        // Util.memoize(Function) uses ConcurrentHashMap.computeIfAbsent;
        // each BiFunction factory likewise caches by its complete argument
        // pair. Serialize bounded admission across the four caches, retaining
        // existing objects without eviction or per-frame identity recycling.
        synchronized(CACHE) {
            JavaRenderType existing=CACHE.get(factory).get(texture);
            if(existing!=null)return existing;
            if(references>=MAX_REFERENCES)throw new IllegalArgumentException("Java render-type reference budget exceeded");
            final Factory selected=factory;
            return CACHE.get(factory).computeIfAbsent(texture,key->new JavaRenderType(selected,key,++references));
        }
    }
}
