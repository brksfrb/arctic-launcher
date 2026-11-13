package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.Xform;
import java.awt.image.BufferedImage;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.List;

import com.arcticlauncher.client.Lang;
/** Owned CPU geometry produced by the shared Java models. This is staging,
 * not an engine Mesh, native allocation, material binding or upload receipt.
 * Positions retain the caller's coordinate space; the native renderer must
 * supply its matching camera, terrain depth, lighting and material states. */
public final class NativeAvatarGeometry {
    public enum Pass {
        GENERAL(0,false,false,false),COSMETIC_BASE(.1f,false,true,false),
        COSMETIC_GLOW(.1f,false,true,false),CAPE_SOLID(0,true,false,false),WING_CUTOUT(.1f,false,true,true),
        SKIN_TRANSLUCENT(.1f,false,true,false),BENT_SKIN(.1f,false,true,false);
        public final float alphaCutout;
        public final boolean cullBackFaces,perFaceLighting,equipmentProjection;
        Pass(float alphaCutout,boolean cullBackFaces,boolean perFaceLighting,boolean equipmentProjection) {
            this.alphaCutout=alphaCutout;this.cullBackFaces=cullBackFaces;
            this.perFaceLighting=perFaceLighting;this.equipmentProjection=equipmentProjection;
        }
        boolean layerGeometry(){return this==COSMETIC_BASE||this==COSMETIC_GLOW||this==CAPE_SOLID||this==WING_CUTOUT||this==BENT_SKIN;}
    }
    private static final int MAX_DRAWS=2048,MAX_VERTICES=400_000,MAX_TEXTURES=32;
    private static final int MAX_TEXTURE_SIDE=4096,MAX_TEXTURE_PIXELS=16_777_216;
    private final List<Batch> batches;
    private final List<Texture> textures;
    private final List<String> actors;

    private NativeAvatarGeometry(List<Batch> batches,List<Texture> textures,List<String> actors) {
        this.batches=Collections.unmodifiableList(new ArrayList<Batch>(batches));
        this.textures=Collections.unmodifiableList(new ArrayList<Texture>(textures));
        this.actors=Collections.unmodifiableList(new ArrayList<String>(actors));
    }
    public List<Batch> batches(){return batches;}
    public List<Texture> textures(){return textures;}
    public List<String> actors(){return actors;}

    /** Frame-local model identity and copied root translation in the packet's
     * declared space. This is not evidence of Java view-space equivalence. */
    public static final class ModelSubmission {
        public final int id;
        public final float x,y,z;
        final GlintSample glint;
        private ModelSubmission(int id,float[] root) {this(id,root[3],root[7],root[11],null);}
        private ModelSubmission(int id,float x,float y,float z,GlintSample glint) {
            this.id=id;this.x=x;this.y=y;this.z=z;this.glint=glint==null?null:glint.copy();
        }
    }

    /** One immutable image snapshot. ARGB is Java's getRGB representation;
     * a native texture uploader must explicitly convert its channel format. */
    public static final class Texture {
        public final int width,height;
        private final int[] argb;
        private Texture(BufferedImage image) {
            width=image.getWidth();height=image.getHeight();
            argb=image.getRGB(0,0,width,height,null,0,width);
        }
        public int[] argb(){return argb.clone();}
    }

    /** Storage order is emission order. Model boundaries survive separately;
     * phase sorting, glow ordering and state selection remain renderer inputs. Vertex data
     * is x/y/z, nx/ny/nz, u/v (eight floats). Colors and packed Java light are
     * separate; tint is unbaked so material/shader multiplication order survives.
     * Quads use Java's (0,1,2),(2,3,0), including padded mesh triangles.
     * Empty batches retain submit/consolidation boundaries. */
    public static final class Batch implements JavaSimplePhases.Submit {
        public final Pass pass;
        public final int actor,texture,vertexCount;
        public final String logicalTextureId;
        public final JavaRenderType renderType;
        public final ModelSubmission modelSubmission;
        public final boolean translucent,emissive,sceneLightKnown,skinVisibilityKnown,layerVisibilityKnown;
        private final float[] vertices,tint,skinOverlay;
        private final int[] colors,lights,indices;
        private Batch(Pending pending) {
            actor=pending.actor;pass=pending.pass;texture=pending.texture;translucent=pending.translucent;emissive=pending.emissive;
            logicalTextureId=pending.logicalTextureId;
            renderType=pending.renderType;
            modelSubmission=pending.modelSubmission;
            sceneLightKnown=pending.sceneLightKnown;
            skinVisibilityKnown=pending.skinVisibilityKnown;
            layerVisibilityKnown=pending.layerVisibilityKnown;
            skinOverlay=pending.skinOverlay==null?null:pending.skinOverlay.clone();
            vertexCount=pending.count;
            vertices=Arrays.copyOf(pending.vertices,vertexCount*8);
            colors=Arrays.copyOf(pending.colors,vertexCount);lights=Arrays.copyOf(pending.lights,vertexCount);
            tint=pending.tint.clone();indices=new int[vertexCount/4*6];
            for(int vertex=0,index=0;vertex<vertexCount;vertex+=4) {
                indices[index++]=vertex;indices[index++]=vertex+1;indices[index++]=vertex+2;
                indices[index++]=vertex+2;indices[index++]=vertex+3;indices[index++]=vertex;
            }
        }
        public float[] vertices(){return vertices.clone();}
        public float[] tint(){return tint.clone();}
        public float[] skinOverlay(){return skinOverlay==null?null:skinOverlay.clone();}
        public int[] colors(){return colors.clone();}
        public int[] lights(){return lights.clone();}
        public int[] indices(){return indices.clone();}
        @Override public Pass phasePass(){return pass;}
        @Override public JavaRenderType phaseType(){return renderType;}
        @Override public Object phaseModel(){return modelSubmission;}
    }

    private static final class Pending {
        final Pass pass;
        final int actor,texture;
        final String logicalTextureId;
        final JavaRenderType renderType;
        ModelSubmission modelSubmission;
        final boolean translucent,emissive,sceneLightKnown,skinVisibilityKnown,layerVisibilityKnown;
        final int sceneLight;
        final float[] tint,skinOverlay;
        float[] vertices=new float[32];
        int[] colors=new int[4],lights=new int[4];
        int count;
        Pending(int actor,Pass pass,int texture,boolean translucent,boolean emissive,float[] tint,Integer sceneLight,float[] skinOverlay,boolean skinVisibilityKnown,boolean layerVisibilityKnown,String logicalTextureId,JavaRenderType renderType) {
            this.actor=actor;this.pass=pass;this.texture=texture;this.translucent=translucent;this.emissive=emissive;this.tint=tint;
            this.sceneLightKnown=sceneLight!=null;this.sceneLight=sceneLight==null?Xform.FULL_BRIGHT:sceneLight;
            this.skinOverlay=skinOverlay;
            this.skinVisibilityKnown=skinVisibilityKnown;
            this.layerVisibilityKnown=layerVisibilityKnown;
            this.logicalTextureId=logicalTextureId;
            this.renderType=renderType;
        }
        void append(float x,float y,float z,int color,float u,float v,int light,float nx,float ny,float nz) {
            if(count==colors.length) {
                int capacity=Math.min(MAX_VERTICES,colors.length*2);
                vertices=Arrays.copyOf(vertices,capacity*8);
                colors=Arrays.copyOf(colors,capacity);lights=Arrays.copyOf(lights,capacity);
            }
            int at=count*8;
            vertices[at]=x;vertices[at+1]=y;vertices[at+2]=z;
            vertices[at+3]=nx;vertices[at+4]=ny;vertices[at+5]=nz;vertices[at+6]=u;vertices[at+7]=v;
            colors[count]=color;lights[count]=light;count++;
        }
    }

    /** A bounded transaction: invalid/partial geometry cannot return a partial
     * frame. No native pointers, allocator headers, bone IDs or enum values are
     * fabricated from these Java arrays. Engine ownership needs a separate copy. */
    static final class Builder implements AvatarDrawTarget {
        private final List<Pending> draws=new ArrayList<Pending>();
        private final List<Texture> textures=new ArrayList<Texture>();
        private final List<String> actors=new ArrayList<String>();
        private final IdentityHashMap<BufferedImage,Integer> textureIds=new IdentityHashMap<BufferedImage,Integer>();
        private final java.util.Map<String,Integer> logicalTextures=new java.util.HashMap<String,Integer>();
        private int actor=-1,vertices,texturePixels;
        private float opacity=1;
        private boolean sealed,failed;
        private Integer sceneLight;
        private boolean skinVisibilityKnown;
        private JavaLayerVisibility layerVisibility;
        private int modelCount,modelFirstDraw;
        private ModelSubmission modelSubmission;

        @Override public void beginModel(float[] root) {
            active();
            if(actor<0||modelSubmission!=null||modelCount>=MAX_DRAWS)throw fail("invalid model submission boundary");
            if(!draws.isEmpty()&&draws.get(draws.size()-1).count%4!=0)throw fail("model starts within a quad");
            if(root==null||root.length!=12)throw fail("model root needs twelve floats");
            for(float value:root)if(!Float.isFinite(value))throw fail("non-finite model root"); // i18n-ignore
            modelSubmission=new ModelSubmission(++modelCount,root);
            modelFirstDraw=draws.size();
        }
        @Override public void endModel() {
            // Also runs in AvatarRenderer's finally block after a failed emit.
            if(sealed||modelSubmission==null)throw new IllegalStateException("no open model submission");
            if(!failed&&!draws.isEmpty()&&draws.get(draws.size()-1).count%4!=0)throw fail("model ended within a quad");
            modelSubmission=null;
        }
        @Override public void modelGlint(GlintSample observed) {
            active();
            if(modelSubmission==null||draws.size()!=modelFirstDraw)
                throw fail("model glint must be selected before its first sink");
            modelSubmission=new ModelSubmission(modelSubmission.id,modelSubmission.x,modelSubmission.y,modelSubmission.z,observed);
        }

        private IllegalArgumentException fail(String message) {
            failed=true;return new IllegalArgumentException(message);
        }
        private void active() {
            if(sealed||failed)throw new IllegalStateException("native geometry transaction is closed");
        }
        void actor(String id) {
            active();
            if(modelSubmission!=null)throw fail("actor changed within a model submission");
            if(!draws.isEmpty()&&draws.get(draws.size()-1).count%4!=0)
                throw fail("native actor ended with an incomplete quad");
            if(id==null||!java.util.UUID.fromString(id).toString().equals(id)||actors.contains(id)||actors.size()>=128)
                throw fail("invalid or duplicate native geometry actor");
            actor=actors.size();actors.add(id);
            sceneLight=null;
            skinVisibilityKnown=false;
            layerVisibility=null;
        }
        @Override public void skinVisibility(JavaSkinVisibility observed) {
            active();
            if(!draws.isEmpty()&&draws.get(draws.size()-1).count%4!=0)throw fail("native skin visibility changed within a quad");
            skinVisibilityKnown=observed!=null;
        }
        @Override public void layerVisibility(JavaLayerVisibility observed) {
            active();
            if(!draws.isEmpty()&&draws.get(draws.size()-1).count%4!=0)throw fail("native layer visibility changed within a quad");
            layerVisibility=observed;
        }
        @Override public int packedLight(Integer observed) {
            active();
            if(!draws.isEmpty()&&draws.get(draws.size()-1).count%4!=0)
                throw fail("native scene light changed within a quad");
            sceneLight=observed;
            return AvatarDrawTarget.super.packedLight(observed);
        }
        private int texture(BufferedImage image) {
            if(image==null)return -1;
            Integer previous=textureIds.get(image);if(previous!=null)return previous;
            int width=image.getWidth(),height=image.getHeight();
            if(width<1||height<1||width>MAX_TEXTURE_SIDE||height>MAX_TEXTURE_SIDE
                    ||textures.size()>=MAX_TEXTURES||(long)texturePixels+(long)width*height>MAX_TEXTURE_PIXELS)
                throw fail("native geometry texture budget exceeded");
            Texture copied=new Texture(image);int id=textures.size();
            textures.add(copied);textureIds.put(image,id);texturePixels+=width*height;return id;
        }
        private int texture(BufferedImage image,String logicalId) {
            if(logicalId==null)return texture(image);
            Integer previous=logicalTextures.get(logicalId);
            if(previous==null){int copied=texture(image);logicalTextures.put(logicalId,copied);return copied;}
            if(image==null) {
                if(previous!=-1)throw fail(Lang.t("platform.java_texture_identifier_changes_between"));
                return -1;
            }
            if(previous==-1)throw fail(Lang.t("platform.java_texture_identifier_changes_between_2"));
            Integer cached=textureIds.get(image);if(previous.equals(cached))return previous;
            Texture before=textures.get(previous),candidate;
            if(cached!=null)candidate=textures.get(cached);
            else {
                if(image.getWidth()!=before.width||image.getHeight()!=before.height)
                    throw fail(Lang.t("platform.java_texture_identifier_changes_image"));
                candidate=new Texture(image);
            }
            if(before.width!=candidate.width||before.height!=candidate.height||!Arrays.equals(before.argb,candidate.argb))
                throw fail(Lang.t("platform.java_texture_identifier_has_conflicting"));
            textureIds.put(image,previous);return previous;
        }
        @Override public Xform sink(float[] pose,BufferedImage image,boolean translucent,boolean emissive,float[] color) {
            return sink(pose,image,translucent,emissive,color,Pass.GENERAL);
        }
        @Override public Xform cosmeticSink(float[] pose,BufferedImage image,boolean emissive,float[] color) {
            return sink(pose,image,true,emissive,color,emissive?Pass.COSMETIC_GLOW:Pass.COSMETIC_BASE);
        }
        @Override public Xform cosmeticSink(float[] pose,BufferedImage image,boolean emissive,float[] color,String textureId) {
            return sink(pose,image,true,emissive,color,emissive?Pass.COSMETIC_GLOW:Pass.COSMETIC_BASE,null,textureId);
        }
        @Override public Xform skinSink(float[] pose,BufferedImage image,boolean previewBlend,float[] color,JavaSkinMaterial material) {
            return skinSink(pose,image,previewBlend,color,material,null);
        }
        @Override public Xform skinSink(float[] pose,BufferedImage image,boolean previewBlend,float[] color,JavaSkinMaterial material,String textureId) {
            if(material==null)return sink(pose,image,previewBlend,false,color,Pass.GENERAL,null,textureId);
            return sink(pose,image,true,false,color,Pass.SKIN_TRANSLUCENT,material.overlayColor(),textureId);
        }
        @Override public Xform bentSkinSink(float[] pose,BufferedImage image,float[] color) {
            return sink(pose,image,true,false,color,Pass.BENT_SKIN);
        }
        @Override public Xform bentSkinSink(float[] pose,BufferedImage image,float[] color,String textureId) {
            return sink(pose,image,true,false,color,Pass.BENT_SKIN,null,textureId);
        }
        @Override public Xform capeSink(float[] pose,BufferedImage image,float[] color) {
            return sink(pose,image,false,false,color,Pass.CAPE_SOLID);
        }
        @Override public Xform capeSink(float[] pose,BufferedImage image,float[] color,String textureId) {
            return sink(pose,image,false,false,color,Pass.CAPE_SOLID,null,textureId);
        }
        @Override public Xform wingSink(float[] pose,BufferedImage image,float[] color) {
            return sink(pose,image,false,false,color,Pass.WING_CUTOUT);
        }
        @Override public Xform wingSink(float[] pose,BufferedImage image,float[] color,String textureId) {
            return sink(pose,image,false,false,color,Pass.WING_CUTOUT,null,textureId);
        }
        private Xform sink(float[] pose,BufferedImage image,boolean translucent,boolean emissive,float[] color,Pass pass) {
            return sink(pose,image,translucent,emissive,color,pass,null);
        }
        private Xform sink(float[] pose,BufferedImage image,boolean translucent,boolean emissive,float[] color,Pass pass,float[] skinOverlay) {
            return sink(pose,image,translucent,emissive,color,pass,skinOverlay,null);
        }
        private Xform sink(float[] pose,BufferedImage image,boolean translucent,boolean emissive,float[] color,Pass pass,float[] skinOverlay,String logicalTextureId) {
            active();
            if(!draws.isEmpty()&&draws.get(draws.size()-1).count%4!=0)
                throw fail("native geometry draw ended with an incomplete quad");
            if(draws.size()>=MAX_DRAWS)throw fail("native geometry draw budget exceeded");
            boolean modelPass=pass==Pass.SKIN_TRANSLUCENT||pass==Pass.CAPE_SOLID||pass==Pass.WING_CUTOUT;
            if(modelSubmission!=null&&modelSubmission.glint!=null&&pass!=Pass.WING_CUTOUT)throw fail("glint metadata requires a wing model");
            if(modelSubmission!=null&&!modelPass&&pass!=Pass.GENERAL)throw fail("custom geometry inside a model submission");
            final JavaRenderType renderType;
            try {WorldFrame.color(color);WorldFrame.color(skinOverlay);JavaTextureId.validate(logicalTextureId);
                renderType=JavaRenderType.forPass(pass,logicalTextureId);
            }catch(RuntimeException error){failed=true;throw error;}
            if((pass==Pass.SKIN_TRANSLUCENT)!=(skinOverlay!=null))throw fail("skin overlay does not match its material role");
            if(pass.layerGeometry()&&layerVisibility!=null) {
                boolean allowed=pass==Pass.WING_CUTOUT?layerVisibility.layersShown()
                    :pass==Pass.CAPE_SOLID?layerVisibility.capeShown():layerVisibility.cosmeticsShown();
                if(!allowed)throw fail("native geometry emits a hidden Java layer"); // i18n-ignore
            }
            boolean layerKnown=pass.layerGeometry()&&layerVisibility!=null&&(pass==Pass.WING_CUTOUT?layerVisibility.layersKnown()
                :pass==Pass.CAPE_SOLID?layerVisibility.capeKnown():layerVisibility.cosmeticsKnown());
            float[] tint=color==null?new float[]{1,1,1,1}:color.clone();tint[3]*=opacity;
            boolean unblended=pass==Pass.CAPE_SOLID||pass==Pass.WING_CUTOUT;
            Pending pending=new Pending(actor,pass,texture(image,logicalTextureId),!unblended&&(translucent||tint[3]<1),emissive,tint,sceneLight,
                skinOverlay==null?null:skinOverlay.clone(),skinVisibilityKnown&&(pass==Pass.SKIN_TRANSLUCENT||pass==Pass.BENT_SKIN),
                layerKnown,logicalTextureId,renderType);
            pending.modelSubmission=modelPass?modelSubmission:null;
            final Xform result;
            try {result=new AvatarPoseSink(pose,pass!=Pass.GENERAL) {
                @Override public void vertex(float vx,float vy,float vz,int argb,float u,float v,int light,float nx,float ny,float nz) {
                    active();
                    // A previously returned sink cannot resume after a later
                    // draw: that would silently reorder translucent layers.
                    if(draws.isEmpty()||draws.get(draws.size()-1)!=pending||pending.actor!=Builder.this.actor)
                        throw fail("native geometry draws must be emitted sequentially");
                    if(pending.modelSubmission!=null&&pending.modelSubmission!=Builder.this.modelSubmission)
                        throw fail("model sink resumed outside its submission");
                    if(vertices>=MAX_VERTICES)throw fail("native geometry vertex budget exceeded");
                    if(pending.sceneLightKnown&&!pending.emissive&&light!=pending.sceneLight)
                        throw fail("native vertex light differs from its scene source");
                    // Match the Java signed-byte normal attribute conversion.
                    // NaN normals from singular Java matrices become zero here,
                    // as in the existing software vertex path.
                    if(pass!=Pass.GENERAL) {
                        nx=JavaEntityLighting.shaderNormal(nx);ny=JavaEntityLighting.shaderNormal(ny);nz=JavaEntityLighting.shaderNormal(nz);
                    }
                    if(!Float.isFinite(vx)||!Float.isFinite(vy)||!Float.isFinite(vz)
                            ||!Float.isFinite(nx)||!Float.isFinite(ny)||!Float.isFinite(nz)||!Float.isFinite(u)||!Float.isFinite(v))
                        throw fail("non-finite native geometry vertex"); // i18n-ignore
                    pending.append(vx,vy,vz,argb,u,v,light,nx,ny,nz);vertices++;
                }
            };}catch(RuntimeException error){failed=true;throw error;}
            draws.add(pending);return result;
        }
        @Override public float pushOpacity(float factor) {
            active();
            if(!Float.isFinite(factor)||factor<0||factor>1)throw fail("invalid actor opacity");
            float previous=opacity;opacity*=factor;return previous;
        }
        @Override public void restoreOpacity(float previous) {
            // AvatarRenderer restores this in a finally block after an emission
            // failure too. Keep the transaction failed without hiding its cause.
            if(sealed)throw new IllegalStateException("native geometry transaction is closed");
            if(!Float.isFinite(previous)||previous<0||previous>1)throw fail("invalid saved opacity");
            opacity=previous;
        }
        NativeAvatarGeometry finish() {
            active();
            if(modelSubmission!=null)throw fail("native geometry has an open model submission");
            List<Batch> result=new ArrayList<Batch>();
            for(Pending pending:draws) {
                if(pending.count%4!=0)throw fail("native geometry contains an incomplete quad");
                result.add(new Batch(pending));
            }
            sealed=true;return new NativeAvatarGeometry(result,textures,actors);
        }
    }
}
