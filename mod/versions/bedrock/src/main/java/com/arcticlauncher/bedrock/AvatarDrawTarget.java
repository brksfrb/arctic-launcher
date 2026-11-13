package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.Xform;
import java.awt.image.BufferedImage;

/** Shared avatar geometry emission, before projection, lighting or GPU upload. */
public interface AvatarDrawTarget {
    /** Begin an avatar's light source. A preview may use full bright when
     * absent; native staging must retain that absence as provenance. */
    default int packedLight(Integer observed) {
        return observed==null?Xform.FULL_BRIGHT:observed;
    }
    default void skinVisibility(JavaSkinVisibility observed) {}
    default void layerVisibility(JavaLayerVisibility observed) {}
    /** One submitModel call, even when its parts emit several sinks. The root
     * uses the caller's declared geometry space, not an inferred Java view. */
    default void beginModel(float[] root) {}
    /** Selected glint metadata for this open wing model, before any sinks.
     * A default target may ignore it; this is no implicit extra draw. */
    default void modelGlint(GlintSample observed) {}
    default void endModel() {}
    Xform sink(float[] pose,BufferedImage texture,boolean translucent,boolean emissive,float[] color);
    default Xform skinSink(float[] pose,BufferedImage texture,boolean previewBlend,float[] color,JavaSkinMaterial material) {
        if(material!=null)throw new UnsupportedOperationException("draw target does not implement explicit skin materials");
        return sink(pose,texture,previewBlend,false,color);
    }
    default Xform skinSink(float[] pose,BufferedImage texture,boolean previewBlend,float[] color,JavaSkinMaterial material,String textureId) {
        return skinSink(pose,texture,previewBlend,color,material);
    }
    Xform cosmeticSink(float[] pose,BufferedImage texture,boolean emissive,float[] color);
    default Xform cosmeticSink(float[] pose,BufferedImage texture,boolean emissive,float[] color,String textureId) {
        return cosmeticSink(pose,texture,emissive,color);
    }
    default Xform bentSkinSink(float[] pose,BufferedImage texture,float[] color) {
        return cosmeticSink(pose,texture,false,color);
    }
    default Xform bentSkinSink(float[] pose,BufferedImage texture,float[] color,String textureId) {
        return bentSkinSink(pose,texture,color);
    }
    default Xform cosmeticSink(float[] pose,BufferedImage texture,boolean emissive) {
        return cosmeticSink(pose,texture,emissive,null);
    }
    Xform capeSink(float[] pose,BufferedImage texture,float[] color);
    Xform wingSink(float[] pose,BufferedImage texture,float[] color);
    default Xform capeSink(float[] pose,BufferedImage texture,float[] color,String textureId) {
        return capeSink(pose,texture,color);
    }
    default Xform wingSink(float[] pose,BufferedImage texture,float[] color,String textureId) {
        return wingSink(pose,texture,color);
    }
    float pushOpacity(float factor);
    void restoreOpacity(float previous);
}
