package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import java.awt.image.BufferedImage;
import java.io.*;
import java.util.*;
import javax.imageio.ImageIO;
import javax.imageio.ImageReader;
import javax.imageio.stream.ImageInputStream;

/** Draws the existing Java skin/rig/cosmetic models through a version-neutral
 * triangle backend. Engine bindings must supply real poses, camera and depth
 * before this can be used to draw on players in a Minecraft world. */
public final class AvatarRenderer {
    // Selected Bedrock wing bones carry completed controller poses/scales.
    // Convert their documented boxes from Bedrock Y-up to local Java Y-down;
    // do not apply Java's unrelated wing offsets/inflation to those matrices.
    // ModelPart face layout supplies the Java material's front/back orientation.
    private static final JavaModelBox LEFT_WING=new JavaModelBox(-10,4,0,10,20,2,0,22,0,64,32,false);
    private static final JavaModelBox RIGHT_WING=new JavaModelBox(0,4,0,10,20,2,0,22,0,64,32,true);

    public interface Cosmetic {
        void emit(AvatarDrawTarget raster,float[][] parts,Fit fit,float seconds,boolean animate,int light);
        /** Compatibility entry for standalone previews without scene light. */
        default void emit(AvatarDrawTarget raster,float[][] parts,Fit fit,float seconds,boolean animate) {
            emit(raster,parts,fit,seconds,animate,raster.packedLight(null));
        }
    }
    public static final class Worn {
        final Cosmetic model;
        final Fit fit;
        public Worn(Cosmetic model,Fit fit) { this.model=Objects.requireNonNull(model);this.fit=Objects.requireNonNull(fit); }
    }
    public static final class Avatar {
        public BufferedImage skin, cape;
        private BufferedImage namedSkin,namedCape;
        private String skinId,capeId;
        /** Retain registration identity only while its exact image binding is
         * selected. Legacy direct field replacement cannot reuse stale names. */
        public void skinTexture(BufferedImage image,String id) {
            JavaTextureId.validate(id);skin=namedSkin=image;skinId=image==null?null:id;
        }
        public void capeTexture(BufferedImage image,String id) {
            JavaTextureId.validate(id);cape=namedCape=image;capeId=image==null?null:id;
        }
        String skinTextureId(){return skin==namedSkin?skinId:null;}
        String capeTextureId(){return cape==namedCape?capeId:null;}
        public float[] skinColor;
        public JavaSkinMaterial skinMaterial;
        public JavaSkinVisibility skinVisibility;
        public JavaLayerVisibility layerVisibility;
        public float[] capeColor;
        /** Java packed block/sky light for this actor draw, or null when no
         * corresponding scene sample is available. Preview fallback is not
         * evidence for a native material or a recorded replay frame. */
        public Integer packedLight;
        /** Native actor fade for worn layers and a missing capeColor. Observed
         * skinColor/capeColor already carry their own alpha. */
        public float opacity=1;
        public boolean slim, reduceMotion;
        public Animation emote;
        public float emoteSeconds, seconds, headYaw, headPitch;
        /** Actual engine limb poses, when available; copied before emote posing. */
        public float[][] partPivots, partRotations;
        public float[][] partMatrices;
        /** Optional owned native target. It remains usable if its separate
         * animation definition for elbow/knee bending has been retired. */
        public float[][] preposedParts;
        public CapePhysics.Motion capeMotion;
        JavaWingPhysics.Motion javaWingMotion;
        GlintSample wingGlint;
        public boolean elytra;
        /** Optional copied Java chest-layer decision. Unknown retains the
         * legacy software pose and cannot authorize a native replacement. */
        public JavaCapeLayer.Selection javaCapeLayer;
        /** Left/right completed bone-to-model matrices. Wing scales are not
         * rigid. A live caller must independently attest native replacement
         * before supplying these to the world renderer. No guessed fallback. */
        public float[][] wingMatrices;
        /** Legacy recorded torso metadata. Wing matrices are model-space
         * observations and are not reparented to this or an emote's torso. */
        public float[] wingReferenceBody;
        /** Only engine-attested attachment poses may draw on a live actor. */
        public int availableParts=63;
        public final List<Worn> worn=new ArrayList<Worn>();
    }

    public static Cosmetic cuboid(Geometry geometry,Animation idle,BufferedImage image,BufferedImage glow) {
        return cuboid(null,geometry,idle,image,glow);
    }
    public static Cosmetic cuboid(String id,Geometry geometry,Animation idle,BufferedImage image,BufferedImage glow) {
        if(image==null || image.getWidth()!=geometry.textureWidth || image.getHeight()!=geometry.textureHeight) {
            throw new IllegalArgumentException("cosmetic texture does not match its geometry");
        }
        if(glow!=null && (glow.getWidth()!=image.getWidth() || glow.getHeight()!=image.getHeight())) {
            throw new IllegalArgumentException("glow texture does not match its cosmetic");
        }
        final List<CuboidModel.Piece> pieces=CuboidModel.bake(geometry,idle);
        final String textureId=id==null?null:JavaTextureId.look("cosmetic/"+id);
        final String glowId=id==null?null:JavaTextureId.look("cosmetic/"+id+"_glow");
        return (raster,parts,fit,seconds,animate,light)-> {
            for(CuboidModel.Piece piece:pieces) {
                if(parts[piece.attach.ordinal()]==null) continue;
                float[] pose=fit(parts[piece.attach.ordinal()],fit);
                // Java's CuboidModel path retains idle motion independently
                // of the mesh/cape motion toggle. Keep an explicit replay time.
                piece.emit(raster.cosmeticSink(pose,image,false,null,textureId),light,idle,seconds);
                if(glow!=null) piece.emit(raster.cosmeticSink(pose,glow,true,null,glowId),Xform.FULL_BRIGHT,idle,seconds);
            }
        };
    }

    public static Cosmetic mesh(final MeshModel mesh) throws IOException {
        return mesh(null,mesh);
    }
    public static Cosmetic mesh(String id,final MeshModel mesh) throws IOException {
        final BufferedImage[] images=new BufferedImage[mesh.images.size()];
        final String[] textureIds=new String[images.length];
        for(int i=0;i<images.length;i++)images[i]=png(mesh.images.get(i));
        if(id!=null)for(int i=0;i<images.length;i++)textureIds[i]=JavaTextureId.look("mesh/"+id+"_"+i);
        final MeshDraw.Group[] groups=MeshDraw.groups(mesh);
        return (raster,parts,fit,seconds,animate,light)-> {
            for(MeshDraw.Group group:groups) {
                if(parts[group.attach.ordinal()]==null) continue;
                float[] pose=fit(parts[group.attach.ordinal()],fit);
                for(int primitive:group.primitives) {
                    MeshModel.Primitive p=mesh.primitives.get(primitive);
                    MeshModel.Material m=p.material<0?null:mesh.materials.get(p.material);
                    int texture=m==null?-1:m.texture,glow=m==null?-1:m.emissiveTexture;
                    BufferedImage image=texture<0?null:images[texture];
                    MeshDraw.emitAt(mesh,primitive,group.attach,animate,raster.cosmeticSink(pose,image,false,null,
                        texture<0?JavaTextureId.MESH_WHITE:textureIds[texture]),light,false,seconds);
                    if(glow>=0) MeshDraw.emitAt(mesh,primitive,group.attach,animate,
                            raster.cosmeticSink(pose,images[glow],true,null,textureIds[glow]),light,true,seconds);
                    if(animate&&mesh.sheen!=null) MeshDraw.emitSheenAt(mesh,primitive,group.attach,
                            raster.cosmeticSink(pose,null,true,null,JavaTextureId.MESH_WHITE),seconds);
                }
            }
        };
    }

    /** Headless preview only: the model's local coordinates determine its camera. */
    public static BufferedImage preview(Avatar avatar,int width,int height,int scale,float yaw,float pitch) {
        float pixels=Math.min(scale,Math.min((height-4)/2.15f,(width-4)/1.25f));
        float[] projection={2*pixels/width,0,0,0, 0,-2*pixels/height,0,0, 0,0,1f/64,0, 0,pixels/height,0.5f,1};
        Raster3D raster=new Raster3D(width,height,projection,null);
        float[] root=rotation(pitch,yaw,0);
        // Rotate about the centre of the body, rather than the neck.
        root[3]=-root[1]*0.5f;root[7]=0.5f-root[5]*0.5f;root[11]=-root[9]*0.5f;
        emit(avatar,raster,root);
        return raster.finish();
    }

    /** The caller owns the world transform; this never infers game state. */
    public static void emit(Avatar avatar,Raster3D raster,float[] modelToWorld) {
        emit(avatar,(AvatarDrawTarget)raster,modelToWorld);
    }

    /** Owned, unprojected batches for a native uploader; no engine calls. */
    public static NativeAvatarGeometry geometry(Avatar avatar,float[] modelToWorld) {
        Objects.requireNonNull(avatar,"avatar");
        if(modelToWorld==null||modelToWorld.length!=12)throw new IllegalArgumentException("expected a 3x4 world pose");
        float[] root=modelToWorld.clone();
        for(float value:root)if(!Float.isFinite(value))throw new IllegalArgumentException("invalid world pose");
        NativeAvatarGeometry.Builder builder=new NativeAvatarGeometry.Builder();
        emit(avatar,builder,root);
        return builder.finish();
    }

    public static void emit(Avatar avatar,AvatarDrawTarget raster,float[] modelToWorld) {
        int light=raster.packedLight(avatar.packedLight);
        raster.skinVisibility(avatar.skinVisibility);
        raster.layerVisibility(avatar.layerVisibility);
        boolean drawLayers=avatar.layerVisibility==null||avatar.layerVisibility.layersShown();
        boolean drawCosmetics=avatar.layerVisibility==null||avatar.layerVisibility.cosmeticsShown();
        boolean drawCape=avatar.layerVisibility==null||avatar.layerVisibility.capeShown();
        boolean expressive=avatar.emote!=null&&Rig.expressive(avatar.emote);
        // WingsLayer has no invisible-player/showCape early return. Arctic's
        // skin mixin installs its loaded cape as an explicit elytra texture,
        // which Java selects before the showCape-controlled cape fallback.
        float[][] baseline=new float[Attach.values().length][];
        for(Attach part:Attach.values()) {
            int i=part.ordinal();
            if(avatar.partMatrices!=null) {
                baseline[i]=avatar.partMatrices[i];
            } else {
                float[] pivot=avatar.partPivots==null?new float[]{part.x,part.y,part.z}:avatar.partPivots[i];
                float[] r=avatar.partRotations==null?new float[3]:avatar.partRotations[i];
                // Native rotations already include head look. Only previews
                // with no engine pose use the separate look inputs.
                if(avatar.partRotations==null && i==Rig.HEAD)r=new float[]{avatar.headPitch,avatar.headYaw,0};
                float[] m=rotation(r[0],r[1],r[2]);
                m[3]=pivot[0]/16;m[7]=pivot[1]/16;m[11]=pivot[2]/16;baseline[i]=m;
            }
        }
        if(avatar.preposedParts!=null) {
            if(avatar.preposedParts.length!=6)throw new IllegalArgumentException("preposed parts need six matrices");
            for(float[] part:avatar.preposedParts)WorldFrame.rigid(part);
        }
        float[][] posed=avatar.preposedParts!=null?avatar.preposedParts:avatar.emote==null?baseline:Rig.poseMatrices(avatar.emote,avatar.emoteSeconds,baseline);
        float[][] parts=new float[baseline.length][];
        for(Attach part:Attach.values()) {
            int i=part.ordinal();
            if((avatar.availableParts&(1<<i))==0) continue;
            parts[i]=multiply(modelToWorld,posed[i]);
        }
        if(avatar.skin!=null) {
            raster.beginModel(modelToWorld);
            try {
            boolean blend=avatar.skinMaterial==null&&translucent(avatar.skin);
            boolean skinSubmitted=false;
            for(Attach part:JavaPlayerMesh.renderParts()) {
                if(parts[part.ordinal()]==null)continue;
                // The Java mixin hides originals only in its expressive rig
                // branch. A legacy joint-only emote still draws its original
                // limb as well as the later custom split geometry.
                if(expressive&&bends(avatar.emote,part))continue;
                if(avatar.skinVisibility!=null&&!avatar.skinVisibility.baseShown(part))continue;
                skinSubmitted=true;
                JavaPlayerMesh.emit(part,avatar.slim,avatar.skinVisibility==null||avatar.skinVisibility.outerShown(part),
                    raster.skinSink(parts[part.ordinal()],avatar.skin,blend,avatar.skinColor,avatar.skinMaterial,avatar.skinTextureId()),light);
            }
            // A submitModel with every part hidden still participates in the
            // phase/consolidation sequence, even though it emits no vertices.
            if(!skinSubmitted)raster.skinSink(modelToWorld,avatar.skin,blend,avatar.skinColor,avatar.skinMaterial,avatar.skinTextureId());
            } finally {raster.endModel();}
        }
        float[] capeColor=avatar.capeColor==null?new float[]{1,1,1,avatar.opacity}:avatar.capeColor;
        // PlayerCapeModel extends PlayerModel, so the same emote tail mixin
        // hides its BODY for a split torso. ModelPart.visible stops child
        // traversal too: its cape disappears, although custom worn attachments
        // still use the body's transform and the separate wing model survives.
        // Do not reuse the normal skin's visibility mask for this other model.
        boolean capeBodyVisible=!expressive
            ||!SkinLimbs.bends(avatar.emote,SkinLimbs.Limb.BODY);
        if(avatar.cape!=null && !avatar.elytra && drawCape
                && avatar.javaCapeLayer!=JavaCapeLayer.Selection.HIDDEN && parts[Rig.BODY]!=null) {
            float[] capeRoot=avatar.javaCapeLayer==JavaCapeLayer.Selection.OFFSET?JavaCapeLayer.offsetRoot(modelToWorld):modelToWorld;
            raster.beginModel(capeRoot);
            try {
                if(capeBodyVisible) {
                    float[] body=avatar.javaCapeLayer==JavaCapeLayer.Selection.OFFSET
                        ?multiply(capeRoot,posed[Rig.BODY]):parts[Rig.BODY];
                    float[] capePose=multiply(body,JavaCapePose.local(avatar.capeMotion));
                    JavaCapeMesh.emit(raster.capeSink(capePose,avatar.cape,capeColor,avatar.capeTextureId()),light);
                } else {
                    // Java still submits this model; ModelPart.visible stops
                    // vertex emission, not its phase/consolidation boundary.
                    raster.capeSink(capeRoot,avatar.cape,capeColor,avatar.capeTextureId());
                }
            } finally {raster.endModel();}
        }
        if(drawLayers && avatar.cape!=null && avatar.elytra && (avatar.javaWingMotion!=null||avatar.wingMatrices!=null)) {
            WorldFrame.wings(avatar.wingMatrices);
            if(avatar.javaWingMotion!=null) {
                // Vanilla WingsLayer uses its own model under the entity root,
                // not the parent player's emote-posed body bone.
                float[] wingRoot=JavaWingModel.layerRoot(modelToWorld);
                raster.beginModel(wingRoot);
                try {raster.modelGlint(avatar.wingGlint);
                    JavaWingModel.emitLayer(raster,wingRoot,avatar.javaWingMotion,avatar.cape,capeColor,light,avatar.capeTextureId());
                } finally {raster.endModel();}
            } else {
                // Older providers/archives already carry complete bone-to-model
                // wing poses. Keep their controller/posture, without attaching
                // the independent equipment model to the player's emote torso.
                raster.beginModel(modelToWorld);
                try {
                    raster.modelGlint(avatar.wingGlint);
                    for(int wing:JavaWingModel.renderParts())(wing==0?LEFT_WING:RIGHT_WING).emit(
                        raster.wingSink(multiply(modelToWorld,avatar.wingMatrices[wing]),avatar.cape,capeColor,avatar.capeTextureId()),light);
                } finally {raster.endModel();}
            }
        }
        if(drawCosmetics) {
            float previousOpacity=raster.pushOpacity(avatar.opacity);
            try {for(Worn worn:avatar.worn) worn.model.emit(raster,parts,worn.fit,avatar.seconds,!avatar.reduceMotion,light);
            } finally {raster.restoreOpacity(previousOpacity);}
        }
        // CosmeticsLayer submits every worn model before drawLimbs. Keep
        // that source order, even when both use entityTranslucent: Java's
        // collector may consolidate and sort their quads together later.
        // Skin tint already contains its own alpha, so emit after restoring
        // the separate worn-layer opacity scope.
        if(drawCosmetics&&avatar.skin!=null&&avatar.emote!=null) {
            for(SkinLimbs.Limb limb:SkinLimbs.Limb.values()) {
                float[] pose=parts[limb.attach.ordinal()];
                if(pose==null||!SkinLimbs.bends(avatar.emote,limb))continue;
                // drawLimbs uses the outer option even when the parent model
                // part was hidden for this emote; base visibility is separate.
                SkinLimbs.piece(limb,avatar.slim,avatar.skinVisibility==null||avatar.skinVisibility.outerShown(limb.attach)).emit(
                    raster.bentSkinSink(pose,avatar.skin,avatar.skinColor,avatar.skinTextureId()),light,
                    avatar.emote.only(limb.joint),avatar.emoteSeconds);
            }
        }
    }

    private static boolean bends(Animation animation,Attach part) {
        for(SkinLimbs.Limb limb:SkinLimbs.Limb.values())if(limb.attach==part)return SkinLimbs.bends(animation,limb);
        return false;
    }

    /** Checked image decode for shared model assets, before allocating pixels. */
    public static BufferedImage png(byte[] bytes) throws IOException {
        if(bytes==null || bytes.length>1024*1024) throw new IOException("model PNG exceeds 1 MiB");
        try(ImageInputStream input=ImageIO.createImageInputStream(new ByteArrayInputStream(bytes))) {
            if(input==null) throw new IOException("invalid image stream");
            Iterator<ImageReader> readers=ImageIO.getImageReaders(input);
            if(!readers.hasNext()) throw new IOException("invalid PNG");
            ImageReader reader=readers.next();
            try {
                if(!"png".equalsIgnoreCase(reader.getFormatName())) throw new IOException("model texture must be PNG");
                reader.setInput(input,true,true);
                int w=reader.getWidth(0),h=reader.getHeight(0);
                if(w<1 || h<1 || w>1024 || h>1024) throw new IOException("model texture dimensions exceed 1024");
                return reader.read(0);
            } finally { reader.dispose(); }
        }
    }
    private static boolean translucent(BufferedImage image) {
        if(!image.getColorModel().hasAlpha()) return false;
        for(int y=0;y<image.getHeight();y++) for(int x=0;x<image.getWidth();x++) {
            int a=image.getRGB(x,y)>>>24;if(a!=0&&a!=255) return true;
        }
        return false;
    }
    public static float[] identity() { return new float[]{1,0,0,0,0,1,0,0,0,0,1,0}; }
    private static float[] rotation(float x,float y,float z) {
        float cx=(float)Math.cos(x),sx=(float)Math.sin(x),cy=(float)Math.cos(y),sy=(float)Math.sin(y),cz=(float)Math.cos(z),sz=(float)Math.sin(z);
        return new float[]{cz*cy,cz*sy*sx-sz*cx,cz*sy*cx+sz*sx,0,sz*cy,sz*sy*sx+cz*cx,sz*sy*cx-cz*sx,0,-sy,cy*sx,cy*cx,0};
    }
    static float[] multiply(float[] a,float[] b) {
        return JavaAffinePose.multiply(a,b);
    }
    private static float[] fit(float[] pose,Fit fit) {
        if(!fit.moves())return pose.clone();
        float[] m=JavaAffinePose.translate(pose,0f,fit.dy()/16f,fit.dz()/16f);
        if(fit.mirror) for(int row=0;row<3;row++) m[row*4]=-m[row*4];
        return m;
    }
}
