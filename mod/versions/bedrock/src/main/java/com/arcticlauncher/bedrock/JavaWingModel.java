package com.arcticlauncher.bedrock;

import java.awt.image.BufferedImage;
import java.util.*;

/** Java 26.2 ElytraModel with its one-pixel deformation and WingsLayer's
 * two-pixel Z offset. Root model poses are independent of the body bone. */
final class JavaWingModel {
    private static final JavaModelBox LEFT=new JavaModelBox(-10,0,0,10,20,2,1,22,0,64,32,false);
    private static final JavaModelBox RIGHT=new JavaModelBox(0,0,0,10,20,2,1,22,0,64,32,true);
    private static final List<Integer> PARTS=parts();
    private JavaWingModel() {}
    static List<Integer> renderParts(){return PARTS;}
    private static List<Integer> parts() {
        // ElytraModel.createLayer adds these to PartDefinition's default map;
        // baking retains its traversal, not the definition's insertion order.
        Map<String,Integer> children=new HashMap<String,Integer>();
        children.put("left_wing",0);children.put("right_wing",1);
        return Collections.unmodifiableList(new ArrayList<Integer>(children.values()));
    }
    private static float[] pose(float[] root,JavaWingPhysics.Motion motion,int wing) {
        boolean right=wing==1;
        float[] part=JavaCapePose.part(motion.x,right?-motion.y:motion.y,right?-motion.z:motion.z,
            right?-5:5,motion.crouching?3:0,0);
        return AvatarRenderer.multiply(root,part);
    }
    /** WingsLayer translates the entity pose before submitting its model.
     * Keep that stage separate from ModelPart's own translation/rotation. */
    static float[] layerRoot(float[] root) {return JavaAffinePose.translate(root,0f,0f,.125f);}
    static void emit(AvatarDrawTarget raster,float[] root,JavaWingPhysics.Motion motion,BufferedImage texture,float[] color) {
        emit(raster,root,motion,texture,color,raster.packedLight(null));
    }
    static void emit(AvatarDrawTarget raster,float[] root,JavaWingPhysics.Motion motion,BufferedImage texture,float[] color,int light) {
        emit(raster,root,motion,texture,color,light,null);
    }
    static void emit(AvatarDrawTarget raster,float[] root,JavaWingPhysics.Motion motion,BufferedImage texture,float[] color,int light,String textureId) {
        emitLayer(raster,layerRoot(root),motion,texture,color,light,textureId);
    }
    static void emitLayer(AvatarDrawTarget raster,float[] root,JavaWingPhysics.Motion motion,BufferedImage texture,float[] color,int light,String textureId) {
        for(int wing:PARTS)(wing==0?LEFT:RIGHT).emit(raster.wingSink(pose(root,motion,wing),texture,color,textureId),light);
    }
    /** Re-emits exactly the base mesh/poses for the separate EQUAL-depth pass.
     * Live/replay callers must first supply observed glint and fog inputs. */
    static void emitGlint(Raster3D raster,float[] root,JavaWingPhysics.Motion motion,JavaReferenceAssets assets,JavaArmorGlint uniforms) {
        float[] layer=layerRoot(root);
        for(int wing:PARTS)(wing==0?LEFT:RIGHT).emit(raster.wingGlintSink(pose(layer,motion,wing),assets,uniforms));
    }
}
