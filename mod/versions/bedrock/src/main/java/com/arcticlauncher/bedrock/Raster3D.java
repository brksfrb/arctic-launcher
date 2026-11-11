package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.Xform;
import java.awt.image.BufferedImage;
import java.awt.image.DataBufferInt;
import java.util.*;

/** Headless triangle backend for the shared player/cosmetic models.
 * Positions enter in world space; the supplied column-major matrix maps them
 * to DirectX clip space (z 0..w). No Minecraft camera or terrain is invented. */
public final class Raster3D implements AvatarDrawTarget {
    private final BufferedImage image;
    private final int width, height;
    private final int[] pixels;
    private final float[] depth, matrix,sortOrigin,equipmentMatrix,modelView;
    private final JavaQuadSorting quadSorting;
    private final JavaEntityLighting lighting;
    private final JavaEntityShader entityShader;
    private final BufferedImage glintArtwork;
    private final WorldFrame phaseWorld;
    private final JavaSimplePhases phaseCollector;
    private final List<SoftwareDraw> submissions=new ArrayList<SoftwareDraw>();
    private final IdentityHashMap<JavaRenderType,SoftwareDraw> frameBindings=new IdentityHashMap<JavaRenderType,SoftwareDraw>();
    private final List<Triangle> blended = new ArrayList<Triangle>();
    private final List<Triangle> glowing = new ArrayList<Triangle>();
    private final List<WingGlint> wingGlints = new ArrayList<WingGlint>();
    private ModelScope model;
    private boolean phaseUnavailable,phaseSealed,phaseFinished;
    private int modelCount,sinkSequence,sinkPendingVertices,unknownWingModels;
    private int triangles;
    private float opacity=1;
    private Integer sceneLight;
    private static final int MAX_TRIANGLES = 200_000;
    private static final class ModelScope {
        final float[] viewTranslation;
        boolean hasSink,hasWingSink,glintKnown;
        int pendingVertices;
        WingGlint glint;
        SoftwareDraw binding;
        ModelScope(float[] viewTranslation){this.viewTranslation=viewTranslation;}
    }
    private static final class WingGlint {
        final JavaArmorGlint uniforms;
        final float[] viewTranslation;
        final List<Triangle> triangles=new ArrayList<Triangle>();
        WingGlint(GlintSample sample) {
            uniforms=new JavaArmorGlint(sample);
            float[] values=sample.values();viewTranslation=Arrays.copyOfRange(values,37,40);
        }
    }
    /** One sink retains empty submits as well as its clipped geometry. The
     * frame's shader/projections/viewport are fixed; all pose, light, tint,
     * overlay and fog variation is already in these vertex attributes. */
    private static final class SoftwareDraw implements JavaSimplePhases.Submit {
        final Pass pass;
        final boolean blend,emissive;
        final BufferedImage texture;
        final JavaRenderType type;
        final ModelScope model;
        final List<Triangle> triangles=new ArrayList<Triangle>();
        SoftwareDraw(Pass pass,boolean blend,boolean emissive,BufferedImage texture,String textureId,ModelScope model) {
            this.pass=pass;this.blend=blend;this.emissive=emissive;this.texture=texture;this.model=model;
            type=JavaRenderType.forPass(phasePass(),textureId);
        }
        @Override public NativeAvatarGeometry.Pass phasePass() {
            switch(pass) {
                case JAVA_BASE:return NativeAvatarGeometry.Pass.COSMETIC_BASE;
                case JAVA_BENT:return NativeAvatarGeometry.Pass.BENT_SKIN;
                case JAVA_GLOW:return NativeAvatarGeometry.Pass.COSMETIC_GLOW;
                case JAVA_SOLID:return NativeAvatarGeometry.Pass.CAPE_SOLID;
                case JAVA_WINGS:return NativeAvatarGeometry.Pass.WING_CUTOUT;
                case JAVA_SKIN:return NativeAvatarGeometry.Pass.SKIN_TRANSLUCENT;
                default:return NativeAvatarGeometry.Pass.GENERAL;
            }
        }
        @Override public JavaRenderType phaseType(){return type;}
        @Override public Object phaseModel(){return model;}
        boolean sameBinding(SoftwareDraw other) {
            // This is this software frame's complete prepared binding, not
            // an inferred Java GPU buffer slice/target/scissor identity.
            return texture==other.texture&&pipeline()==other.pipeline()&&blend==other.blend&&emissive==other.emissive;
        }
        Pass pipeline(){return pass==Pass.JAVA_BENT||pass==Pass.JAVA_SKIN?Pass.JAVA_BASE:pass;}
        int bindingSlot(){return pipeline().ordinal()*4+(blend?2:0)+(emissive?1:0);}
    }
    private static final class PreparedDraw {
        final SoftwareDraw binding;
        final List<Triangle> triangles=new ArrayList<Triangle>();
        PreparedDraw(SoftwareDraw binding){this.binding=binding;}
    }
    private static final class SkinModel {
        final ModelScope model;
        final SoftwareDraw binding;
        final List<Triangle> triangles=new ArrayList<Triangle>();
        SkinModel(SoftwareDraw first){model=first.model;binding=first;}
    }
    private enum Pass {
        GENERAL(0),JAVA_BASE(.1f),JAVA_BENT(.1f),JAVA_GLOW(.1f),JAVA_SOLID(0),JAVA_WINGS(.1f),JAVA_ARMOR_GLINT(.1f),JAVA_SKIN(.1f);
        final float alphaCutout;
        Pass(float alphaCutout){this.alphaCutout=alphaCutout;}
        boolean unblended(){return this==JAVA_SOLID||this==JAVA_WINGS;}
        boolean perFaceLighting(){return this==JAVA_BASE||this==JAVA_BENT||this==JAVA_GLOW||this==JAVA_WINGS||this==JAVA_SKIN;}
        boolean writesDepth(){return unblended()||this==JAVA_BASE||this==JAVA_BENT||this==JAVA_SKIN;}
    }

    private static final class Vertex {
        final float[] a; // clip4, UV2, front RGB/alpha, back RGB, fog2, lightmap RGBA, overlay RGBA
        Vertex(float[] a) { this.a = a; }
        Vertex mix(Vertex other, float t) {
            float[] out = new float[a.length];
            for (int i = 0; i < out.length; i++) out[i] = a[i] + (other.a[i] - a[i]) * t;
            return new Vertex(out);
        }
    }
    private static final class Triangle {
        final Vertex a, b, c;
        final BufferedImage texture;
        final Pass pass;
        final JavaArmorGlint glint;
        final float quadDistance;
        final Float projectionKey;
        Triangle(Vertex a, Vertex b, Vertex c, BufferedImage texture,Pass pass,float quadDistance,JavaArmorGlint glint,Float projectionKey) {
            this.a = a; this.b = b; this.c = c; this.texture = texture;this.pass=pass;this.quadDistance=quadDistance;this.glint=glint;
            this.projectionKey=projectionKey;
        }
        float distance() { return Float.isFinite(quadDistance)?quadDistance:a.a[2]/a.a[3]+b.a[2]/b.a[3]+c.a[2]/c.a[3]; }
    }

    public Raster3D(int width, int height, float[] viewProjection, float[] terrainDepth) {
        this(width,height,viewProjection,terrainDepth,null);
    }
    /** sortOrigin must share the emitted positions' coordinate space. Null
     * retains the legacy projected-depth ordering for unspecified adapters. */
    public Raster3D(int width,int height,float[] viewProjection,float[] terrainDepth,float[] sortOrigin) {
        this(width,height,viewProjection,terrainDepth,sortOrigin,null);
    }
    Raster3D(int width,int height,float[] viewProjection,float[] terrainDepth,float[] sortOrigin,JavaEntityLighting lighting) {
        this(width,height,viewProjection,terrainDepth,sortOrigin,lighting,null);
    }
    Raster3D(int width,int height,float[] viewProjection,float[] terrainDepth,float[] sortOrigin,JavaEntityLighting lighting,float[] equipmentProjection) {
        this(width,height,viewProjection,terrainDepth,sortOrigin,lighting,equipmentProjection,null);
    }
    Raster3D(int width,int height,float[] viewProjection,float[] terrainDepth,float[] sortOrigin,JavaEntityLighting lighting,float[] equipmentProjection,JavaEntityShader entityShader) {
        this(width,height,viewProjection,terrainDepth,sortOrigin,lighting,equipmentProjection,entityShader,null);
    }
    Raster3D(int width,int height,float[] viewProjection,float[] terrainDepth,float[] sortOrigin,JavaEntityLighting lighting,float[] equipmentProjection,JavaEntityShader entityShader,JavaReferenceAssets assets) {
        this(width,height,viewProjection,terrainDepth,sortOrigin,lighting,equipmentProjection,entityShader,assets,null,null);
    }
    Raster3D(int width,int height,float[] viewProjection,float[] terrainDepth,float[] sortOrigin,JavaEntityLighting lighting,float[] equipmentProjection,JavaEntityShader entityShader,JavaReferenceAssets assets,WorldFrame phaseWorld,JavaSimplePhases phaseCollector) {
        if (width < 1 || height < 1 || width > 3840 || height > 2160 || viewProjection == null
                || viewProjection.length != 16 || terrainDepth != null && terrainDepth.length != width * height) {
            throw new IllegalArgumentException("invalid 3D surface");
        }
        for (float v : viewProjection) if (!Float.isFinite(v)) throw new IllegalArgumentException("non-finite camera");
        if(equipmentProjection!=null) {
            if(equipmentProjection.length!=16)throw new IllegalArgumentException("invalid equipment projection");
            for(float v:equipmentProjection)if(!Float.isFinite(v))throw new IllegalArgumentException("non-finite equipment camera");
        }
        if(sortOrigin!=null) {
            if(sortOrigin.length!=3)throw new IllegalArgumentException("invalid translucent sort origin");
            for(float v:sortOrigin)if(!Float.isFinite(v)||Math.abs(v)>32_000_000)throw new IllegalArgumentException("invalid translucent sort origin");
        }
        this.sortOrigin=sortOrigin==null?null:sortOrigin.clone();
        this.lighting=lighting;
        this.entityShader=entityShader;
        glintArtwork=assets==null?null:assets.armorGlint();
        if((phaseWorld==null)!=(phaseCollector==null))throw new IllegalArgumentException("incomplete software phase binding");
        this.phaseWorld=phaseWorld;this.phaseCollector=phaseCollector;
        phaseUnavailable=phaseWorld!=null&&sortOrigin==null;
        matrix=viewProjection.clone();
        modelView=phaseWorld==null?null:phaseWorld.copyModelView(matrix);
        quadSorting=phaseWorld==null?null:phaseWorld.copyQuadSorting(matrix);
        equipmentMatrix=equipmentProjection==null?null:equipmentProjection.clone();
        this.width = width; this.height = height;
        image = new BufferedImage(width, height, BufferedImage.TYPE_INT_ARGB);
        pixels = ((DataBufferInt) image.getRaster().getDataBuffer()).getData();
        depth = new float[width * height];
        Arrays.fill(depth, 1f);
        if (terrainDepth != null) {
            for (int i = 0; i < depth.length; i++) {
                float d = terrainDepth[i];
                if (!Float.isFinite(d) || d < 0 || d > 1) throw new IllegalArgumentException("invalid terrain depth");
                depth[i] = d;
            }
        }
    }

    @Override public int packedLight(Integer observed) {
        sceneLight=observed;
        return AvatarDrawTarget.super.packedLight(observed);
    }

    @Override public void beginModel(float[] root) {
        if(phaseSealed||model!=null||modelCount>=2048||root==null||root.length!=12||phaseWorld!=null&&sinkPendingVertices!=0)
            throw new IllegalArgumentException("invalid software model boundary");
        for(float value:root)if(!Float.isFinite(value))throw new IllegalArgumentException("non-finite software model root");
        model=new ModelScope(JavaModelView.translation(modelView,root));modelCount++;
    }
    @Override public void modelGlint(GlintSample observed) {
        if(model==null||model.hasSink)throw new IllegalArgumentException("select software glint before the model's first sink");
        if(observed!=null&&observed.selected&&(glintArtwork==null||equipmentMatrix==null))
            throw new IllegalArgumentException("selected software glint needs verified artwork and an equipment camera");
        model.glintKnown=observed!=null;
        model.glint=observed!=null&&observed.selected?new WingGlint(observed):null;
    }
    @Override public void endModel() {
        if(model==null)throw new IllegalArgumentException("no open software model");
        ModelScope completed=model;model=null;
        if(completed.pendingVertices!=0)throw new IllegalArgumentException("software model ends within a quad");
        if(completed.hasWingSink&&!completed.glintKnown)unknownWingModels++;
        // Retain even an empty selected model: it participates in unstable
        // model ordering, unlike an explicitly absent or unknown pass.
        if(completed.glint!=null)wingGlints.add(completed.glint);
    }

    /** Collects the quads emitted by CuboidModel and MeshDraw. */
    public Xform sink(final float[] pose, final BufferedImage texture, final boolean translucent, final boolean emissive) {
        return sink(pose,texture,translucent,emissive,null);
    }
    public Xform sink(final float[] pose, final BufferedImage texture, final boolean translucent, final boolean emissive,float[] color) {
        return sink(pose,texture,translucent,emissive,color,Pass.GENERAL);
    }
    /** Arctic's Java 26.2 entity-translucent passes: cutout tests the sampled
     * texture before tint/fade; base writes depth, ordered glow only tests it. */
    public Xform cosmeticSink(float[] pose,BufferedImage texture,boolean emissive) {
        return cosmeticSink(pose,texture,emissive,null);
    }
    public Xform cosmeticSink(float[] pose,BufferedImage texture,boolean emissive,float[] color) {
        return sink(pose,texture,true,emissive,color,emissive?Pass.JAVA_GLOW:Pass.JAVA_BASE);
    }
    @Override public Xform cosmeticSink(float[] pose,BufferedImage texture,boolean emissive,float[] color,String textureId) {
        return sink(pose,texture,true,emissive,color,emissive?Pass.JAVA_GLOW:Pass.JAVA_BASE,null,null,textureId);
    }
    @Override public Xform bentSkinSink(float[] pose,BufferedImage texture,float[] color) {
        return sink(pose,texture,true,false,color,Pass.JAVA_BENT);
    }
    @Override public Xform bentSkinSink(float[] pose,BufferedImage texture,float[] color,String textureId) {
        return sink(pose,texture,true,false,color,Pass.JAVA_BENT,null,null,textureId);
    }
    @Override public Xform skinSink(float[] pose,BufferedImage texture,boolean previewBlend,float[] color,JavaSkinMaterial material) {
        if(material==null)return sink(pose,texture,previewBlend,false,color,Pass.GENERAL);
        return sink(pose,texture,true,false,color,Pass.JAVA_SKIN,null,material.overlayColor());
    }
    @Override public Xform skinSink(float[] pose,BufferedImage texture,boolean previewBlend,float[] color,JavaSkinMaterial material,String textureId) {
        if(material==null)return sink(pose,texture,previewBlend,false,color,Pass.GENERAL,null,null,textureId);
        return sink(pose,texture,true,false,color,Pass.JAVA_SKIN,null,material.overlayColor(),textureId);
    }
    /** CapeLayer's entitySolid pipeline: culled, unblended, no alpha cutout,
     * front-normal lighting and depth writes even for transparent texels. */
    public Xform capeSink(float[] pose,BufferedImage texture,float[] color) {
        return sink(pose,texture,false,false,color,Pass.JAVA_SOLID);
    }
    @Override public Xform capeSink(float[] pose,BufferedImage texture,float[] color,String textureId) {
        return sink(pose,texture,false,false,color,Pass.JAVA_SOLID,null,null,textureId);
    }
    /** WingsLayer's armorCutoutNoCull material. Layering requires a matching
     * native camera pair; lightmap and equipment glint remain separate. */
    public Xform wingSink(float[] pose,BufferedImage texture,float[] color) {
        return sink(pose,texture,false,false,color,Pass.JAVA_WINGS);
    }
    @Override public Xform wingSink(float[] pose,BufferedImage texture,float[] color,String textureId) {
        return sink(pose,texture,false,false,color,Pass.JAVA_WINGS,null,null,textureId);
    }
    /** Explicit-input pass; call only after the owning wing base has written
     * depth, in the same Position space. No live clock/fog fallback exists. */
    Xform wingGlintSink(float[] pose,JavaReferenceAssets assets,JavaArmorGlint uniforms) {
        if(assets==null||assets.armorGlint()==null||uniforms==null||equipmentMatrix==null)
            throw new IllegalArgumentException("wing glint needs verified artwork, copied uniforms and an equipment camera");
        return sink(pose,assets.armorGlint(),false,false,null,Pass.JAVA_ARMOR_GLINT,uniforms);
    }
    private Xform sink(float[] inputPose,final BufferedImage texture,final boolean translucent,final boolean emissive,float[] color,final Pass pass) {
        return sink(inputPose,texture,translucent,emissive,color,pass,null);
    }
    private Xform sink(float[] inputPose,final BufferedImage texture,final boolean translucent,final boolean emissive,float[] color,final Pass pass,final JavaArmorGlint glint) {
        return sink(inputPose,texture,translucent,emissive,color,pass,glint,null);
    }
    private Xform sink(float[] inputPose,final BufferedImage texture,final boolean translucent,final boolean emissive,float[] color,final Pass pass,final JavaArmorGlint glint,float[] skinOverlay) {
        return sink(inputPose,texture,translucent,emissive,color,pass,glint,skinOverlay,null);
    }
    private Xform sink(float[] inputPose,final BufferedImage texture,final boolean translucent,final boolean emissive,float[] color,final Pass pass,final JavaArmorGlint glint,float[] skinOverlay,String textureId) {
        if(phaseSealed)throw new IllegalArgumentException("software phase frame is sealed");
        if(phaseWorld!=null&&sinkPendingVertices!=0)throw new IllegalArgumentException("software sink starts within a quad");
        if (inputPose == null || inputPose.length != 12) throw new IllegalArgumentException("expected a 3x4 pose");
        final float[] pose=inputPose.clone();
        for (float f : pose) if (!Float.isFinite(f)) throw new IllegalArgumentException("invalid pose");
        WorldFrame.color(color);
        WorldFrame.color(skinOverlay);
        if((pass==Pass.JAVA_SKIN)!=(skinOverlay!=null))throw new IllegalArgumentException("skin overlay does not match its software material");
        final float[] selectedOverlay=skinOverlay==null?null:skinOverlay.clone();
        final float red=color==null?1:color[0],green=color==null?1:color[1],blue=color==null?1:color[2];
        final float alpha=(color==null?1:color[3])*opacity;
        final boolean blend=glint==null&&!pass.unblended()&&(translucent||alpha<1);
        final boolean entity=entityShader!=null&&pass!=Pass.GENERAL&&glint==null;
        final float[] vertexOverlay=entity&&pass!=Pass.JAVA_WINGS
            ?(selectedOverlay==null?entityShader.overlayColor():selectedOverlay):new float[4];
        final Integer lightSource=sceneLight;
        if(entity&&!emissive&&lightSource==null)
            throw new IllegalArgumentException("entity lightmap shading needs corresponding packed scene light");
        final float[] clipMatrix=(pass==Pass.JAVA_WINGS||glint!=null)&&equipmentMatrix!=null?equipmentMatrix:matrix;
        final ModelScope owner=model;
        if(owner!=null) {
            if(owner.pendingVertices!=0)throw new IllegalArgumentException("software sink starts within a quad");
            if(owner.glintKnown&&pass!=Pass.JAVA_WINGS)throw new IllegalArgumentException("glint metadata requires a wing model");
            owner.hasSink=true;
            if(pass==Pass.JAVA_WINGS)owner.hasWingSink=true;
        }
        final WingGlint selected=owner==null?null:owner.glint;
        SoftwareDraw pendingDraw=null;
        if(phaseWorld!=null&&!phaseUnavailable) {
            if(submissions.size()>=2048)abandonPhases();
            else {
                SoftwareDraw candidate=new SoftwareDraw(pass,blend,emissive,texture,textureId,owner);
                SoftwareDraw previous=frameBindings.get(candidate.type);
                if(candidate.type==null||previous!=null&&!previous.sameBinding(candidate)
                        ||owner!=null&&owner.binding!=null&&!owner.binding.sameBinding(candidate))abandonPhases();
                else {
                    if(previous==null)frameBindings.put(candidate.type,candidate);
                    if(owner!=null&&owner.binding==null)owner.binding=candidate;
                    submissions.add(candidate);pendingDraw=candidate;
                }
            }
        }
        final SoftwareDraw staged=pendingDraw;
        final int sequence=++sinkSequence;
        return new AvatarPoseSink(pose,pass!=Pass.GENERAL&&glint==null,pass!=Pass.GENERAL) {
            private final Vertex[] quad = new Vertex[4];
            private final Vertex[] glintQuad=selected==null?null:new Vertex[4];
            private final float[] diagonal=new float[6];
            private final float[] lightColor={1,1,1,1},fogDistance=new float[2];
            private int count;
            private boolean invalid;
            @Override public void vertex(float vx, float vy, float vz, int argb, float u, float v, int light, float nx, float ny, float nz) {
                if(phaseSealed)throw new IllegalArgumentException("software phase frame is sealed");
                if((owner!=null&&model!=owner)||((owner!=null||phaseWorld!=null)&&sequence!=sinkSequence))
                    throw new IllegalArgumentException("software model sink resumed outside its submission");
                if(entity&&!emissive&&light!=lightSource)
                    throw new IllegalArgumentException("entity vertex light differs from its scene source");
                if(pass!=Pass.GENERAL&&glint==null) {
                    nx=JavaEntityLighting.shaderNormal(nx);ny=JavaEntityLighting.shaderNormal(ny);nz=JavaEntityLighting.shaderNormal(nz);
                }
                float shade = emissive ? 1f : Math.min(1f, 0.68f + 0.25f * Math.abs(nz) + 0.07f * Math.max(0, -ny));
                float back=shade;
                if(entity) {
                    shade=entityShader.shade(nx,ny,nz,true);
                    back=pass.perFaceLighting()?entityShader.shade(nx,ny,nz,false):shade;
                    if(!emissive)entityShader.sampleLightmap(light,lightColor);
                } else if(pass!=Pass.GENERAL&&lighting!=null&&glint==null) {
                    // Emissive retains cardinal lighting. Java translucent
                    // and armor-cutout passes have PER_FACE_LIGHTING colours.
                    shade=lighting.shade(nx,ny,nz,true);
                    back=pass.perFaceLighting()?lighting.shade(nx,ny,nz,false):shade;
                }
                if(glint!=null) {float originalU=u;u=glint.u(originalU,v);v=glint.v(originalU,v);shade=back=1;}
                float spherical=0,cylindrical=0;
                if(glint!=null)glint.fogDistance(vx,vy,vz,fogDistance);
                else if(entity)entityShader.fogDistance(vx,vy,vz,fogDistance);
                if(glint!=null||entity) {spherical=fogDistance[0];cylindrical=fogDistance[1];}
                float[] p = new float[] {
                    clipMatrix[0]*vx+clipMatrix[4]*vy+clipMatrix[8]*vz+clipMatrix[12],
                    clipMatrix[1]*vx+clipMatrix[5]*vy+clipMatrix[9]*vz+clipMatrix[13],
                    clipMatrix[2]*vx+clipMatrix[6]*vy+clipMatrix[10]*vz+clipMatrix[14],
                    clipMatrix[3]*vx+clipMatrix[7]*vy+clipMatrix[11]*vz+clipMatrix[15], u,v,
                    ((argb>>>16)&255)/255f*shade*red, ((argb>>>8)&255)/255f*shade*green, (argb&255)/255f*shade*blue, (argb>>>24)/255f*alpha,
                    ((argb>>>16)&255)/255f*back*red, ((argb>>>8)&255)/255f*back*green, (argb&255)/255f*back*blue,
                    spherical,cylindrical,lightColor[0],lightColor[1],lightColor[2],lightColor[3],
                    vertexOverlay[0],vertexOverlay[1],vertexOverlay[2],vertexOverlay[3]
                };
                for (float value : p) if (!Float.isFinite(value)) invalid=true;
                if(selected!=null) {
                    // Same emitted position and exact clip coordinates as
                    // the base; no mesh/pose reconstruction or second skinning.
                    float[] attributes=p.clone();
                    attributes[4]=selected.uniforms.u(u,v);attributes[5]=selected.uniforms.v(u,v);
                    selected.uniforms.fogDistance(vx,vy,vz,fogDistance);
                    attributes[13]=fogDistance[0];attributes[14]=fogDistance[1];
                    if(!Float.isFinite(attributes[4])||!Float.isFinite(attributes[5])
                            ||!Float.isFinite(attributes[13])||!Float.isFinite(attributes[14]))
                        throw new IllegalArgumentException("selected glint overflows its vertex inputs");
                    glintQuad[count]=new Vertex(attributes);
                }
                if(count==0||count==2) {
                    int at=count==0?0:3;diagonal[at]=vx;diagonal[at+1]=vy;diagonal[at+2]=vz;
                }
                quad[count++] = new Vertex(p);
                if(owner!=null)owner.pendingVertices=count%4;
                if(phaseWorld!=null)sinkPendingVertices=count%4;
                if (count == 4) {
                    if (!invalid) {
                        float distance=Float.NaN;
                        if(sortOrigin!=null) {
                            // MeshData.decodeQuadCentroids uses vertices 0/2,
                            // including MeshDraw's padded triangle quads.
                            float dx=(diagonal[0]+diagonal[3])/2-sortOrigin[0];
                            float dy=(diagonal[1]+diagonal[4])/2-sortOrigin[1];
                            float dz=(diagonal[2]+diagonal[5])/2-sortOrigin[2];
                            distance=dx*dx+(dy*dy+dz*dz);
                            invalid=!Float.isFinite(distance);
                        }
                        if(!invalid) {
                            // Upload sorting reads original buffer positions,
                            // not clip coordinates or a reconstructed view.
                            Float projectionKey=staged==null||quadSorting==null?null:quadSorting.key(diagonal);
                            submit(quad[0],quad[1],quad[2],texture,blend,emissive,pass,distance,glint,staged==null?null:staged.triangles,projectionKey);
                            // MeshData.SortState and the native packet emit
                            // 2,3,0. Preserve the primitive's starting vertex
                            // before clipping/interpolation, not just winding.
                            submit(quad[2],quad[3],quad[0],texture,blend,emissive,pass,distance,glint,staged==null?null:staged.triangles,projectionKey);
                            if(selected!=null) {
                                submit(glintQuad[0],glintQuad[1],glintQuad[2],glintArtwork,false,false,Pass.JAVA_ARMOR_GLINT,
                                    Float.NaN,selected.uniforms,selected.triangles);
                                submit(glintQuad[2],glintQuad[3],glintQuad[0],glintArtwork,false,false,Pass.JAVA_ARMOR_GLINT,
                                    Float.NaN,selected.uniforms,selected.triangles);
                            }
                        }
                    }
                    count=0;invalid=false;
                }
            }
        };
    }
    public float pushOpacity(float factor) {
        if(!Float.isFinite(factor)||factor<0||factor>1)throw new IllegalArgumentException("invalid actor opacity");
        float previous=opacity;opacity*=factor;return previous;
    }
    public void restoreOpacity(float previous){opacity=previous;}

    private static float plane(Vertex v, int p) {
        float[] a=v.a;
        switch(p) {
            case 0:return a[0]+a[3]; case 1:return a[3]-a[0];
            case 2:return a[1]+a[3]; case 3:return a[3]-a[1];
            case 4:return a[2]; case 5:return a[3]-a[2]; default:return a[3]-0.0001f;
        }
    }
    private void submit(Vertex a,Vertex b,Vertex c,BufferedImage texture,boolean translucent,boolean emissive,Pass pass,float quadDistance,JavaArmorGlint glint) {
        submit(a,b,c,texture,translucent,emissive,pass,quadDistance,glint,null);
    }
    private void submit(Vertex a,Vertex b,Vertex c,BufferedImage texture,boolean translucent,boolean emissive,Pass pass,float quadDistance,JavaArmorGlint glint,List<Triangle> deferred) {
        submit(a,b,c,texture,translucent,emissive,pass,quadDistance,glint,deferred,null);
    }
    private void submit(Vertex a,Vertex b,Vertex c,BufferedImage texture,boolean translucent,boolean emissive,Pass pass,float quadDistance,JavaArmorGlint glint,List<Triangle> deferred,Float projectionKey) {
        List<Vertex> polygon=new ArrayList<Vertex>(Arrays.asList(a,b,c));
        for(int p=0;p<7 && !polygon.isEmpty();p++) {
            List<Vertex> out=new ArrayList<Vertex>();
            Vertex previous=polygon.get(polygon.size()-1); float before=plane(previous,p);
            for(Vertex at:polygon) {
                float after=plane(at,p);
                if((before>=0)!=(after>=0)) out.add(previous.mix(at,before/(before-after)));
                if(after>=0) out.add(at);
                previous=at;before=after;
            }
            polygon=out;
        }
        for(int i=1;i+1<polygon.size();i++) {
            if (++triangles > MAX_TRIANGLES) throw new IllegalArgumentException("world geometry exceeds frame budget");
            Triangle t=new Triangle(polygon.get(0),polygon.get(i),polygon.get(i+1),texture,pass,quadDistance,glint,projectionKey);
            if(deferred!=null)deferred.add(t);
            else if(emissive) glowing.add(t);
            else if(translucent) blended.add(t); else draw(t,false);
        }
    }

    /** Ordered cosmetic layers: base first, then glow. Both test terrain and
     * player depth; Java cosmetic base fragments additionally write depth. */
    public BufferedImage finish() {
        if(phaseFinished)return image;
        if(phaseSealed)throw new IllegalArgumentException("software phase frame failed or is already finishing");
        if(model!=null)throw new IllegalArgumentException("software frame has an open model submission");
        if(phaseWorld!=null&&sinkPendingVertices!=0)throw new IllegalArgumentException("software phase frame ends within a quad");
        // Unknown wing selection may add another participant and change
        // unstable ties. Never sort a known active subset as a full phase.
        if(!wingGlints.isEmpty()&&unknownWingModels!=0)
            throw new IllegalArgumentException("software glint phase has unknown wing selections");
        phaseSealed=phaseWorld!=null;
        JavaSimplePhases.Result phases=null;
        if(phaseCollector!=null) {
            if(phaseUnavailable)phaseCollector.reset();
            else phases=phaseCollector.collect(phaseWorld,submissions,submissions);
        }
        if(phases!=null) {
            phases.requireSource(phaseWorld,submissions);
            drawSimple(phases,0,0,false);
            drawSkinModels();
            drawSimple(phases,0,8,true);
        } else {
            // Unknown names/general variants cannot create factory identities.
            // Retain the legacy renderer and invalidate owned collector history.
            for(SoftwareDraw submit:submissions)enqueueLegacy(submit);
            drawSorted(blended);
        }
        // Owned order-1 translucent model phase, after order 0 and before
        // order-1 custom glow. No glint per-quad sortOnUpload is applied.
        float[][] translations=new float[wingGlints.size()][];
        for(int i=0;i<translations.length;i++)translations[i]=wingGlints.get(i).viewTranslation;
        for(int index:JavaModelOrder.sort(translations))for(Triangle t:wingGlints.get(index).triangles)draw(t,false);
        if(phases!=null)drawSimple(phases,1,8,true);else drawSorted(glowing);
        blended.clear();wingGlints.clear();glowing.clear();submissions.clear();frameBindings.clear();
        phaseFinished=phaseWorld!=null;return image;
    }
    private void drawSorted(List<Triangle> triangles) {
        drawSorted(triangles,false);
    }
    private void drawSorted(List<Triangle> triangles,boolean projectionAware) {
        // A prepared draw uses one comparator for its complete geometry. Never
        // sort a known subset or substitute a key from a different space.
        boolean known=projectionAware;
        if(known)for(Triangle triangle:triangles)if(triangle.projectionKey==null){known=false;break;}
        if(known)Collections.sort(triangles,(a,b)->Float.compare(b.projectionKey,a.projectionKey));
        else Collections.sort(triangles,(a,b)->Float.compare(b.distance(),a.distance()));
        for(Triangle triangle:triangles)draw(triangle,true);
    }
    private void drawSkinModels() {
        List<SkinModel> models=new ArrayList<SkinModel>();SkinModel last=null;
        for(SoftwareDraw submit:submissions) {
            if(submit.pass!=Pass.JAVA_SKIN)continue;
            if(submit.model==null||submit.model.viewTranslation==null) {
                // A combined projection alone cannot recover a model view.
                // Retain the older skin renderer without inventing a pose.
                List<Triangle> skins=new ArrayList<Triangle>();
                for(SoftwareDraw skin:submissions)if(skin.pass==Pass.JAVA_SKIN)skins.addAll(skin.triangles);
                drawSorted(skins);return;
            }
            if(last==null||last.model!=submit.model){last=new SkinModel(submit);models.add(last);}
            last.triangles.addAll(submit.triangles);
        }
        float[][] poses=new float[models.size()][];
        for(int i=0;i<poses.length;i++)poses[i]=models.get(i).model.viewTranslation;
        List<PreparedDraw> draws=new ArrayList<PreparedDraw>();PreparedDraw previous=null;JavaRenderType previousType=null;
        for(int index:JavaModelOrder.sort(poses)) {
            SkinModel submit=models.get(index);
            // Strict model outputs permit consecutive consolidation only.
            // Never merge around an intervening different render reference.
            if(previous==null||previousType!=submit.binding.type||!previous.binding.sameBinding(submit.binding)) {
                previous=new PreparedDraw(submit.binding);draws.add(previous);
            }
            previous.triangles.addAll(submit.triangles);previousType=submit.binding.type;
        }
        for(PreparedDraw prepared:draws)drawSorted(prepared.triangles,true);
    }
    private void enqueueLegacy(SoftwareDraw submit) {
        for(Triangle triangle:submit.triangles) {
            if(submit.emissive)glowing.add(triangle);
            else if(submit.blend)blended.add(triangle);else draw(triangle,false);
        }
    }
    private void abandonPhases() {
        // An unsupported or oversized collector scope need not remove the
        // old renderer's geometry. Reuse already-clipped triangles exactly
        // once, then collect future sinks through the original queues.
        phaseUnavailable=true;
        boolean moved=false;
        try {
            phaseCollector.reset();
            for(SoftwareDraw submit:submissions)enqueueLegacy(submit);
            submissions.clear();frameBindings.clear();moved=true;
        } finally {if(!moved)phaseSealed=true;}
    }
    private void drawSimple(JavaSimplePhases.Result phases,int order,int stage,boolean sortQuads) {
        for(JavaSimplePhases.Phase phase:phases.phases) {
            if(phase.order!=order||phase.stage!=stage)continue;
            // Simple outputs allow prepared binding reuse across their bucket
            // traversal. QUADS is disconnected, so these factory recipes can
            // consolidate. Keep first prepared encounter and all empty submits.
            List<PreparedDraw> draws=new ArrayList<PreparedDraw>();
            IdentityHashMap<BufferedImage,PreparedDraw[]> bindings=new IdentityHashMap<BufferedImage,PreparedDraw[]>();
            for(int index:phase.batches()) {
                if(index<0||index>=submissions.size())throw new IllegalArgumentException("software phase references a missing submit");
                SoftwareDraw submit=submissions.get(index);
                PreparedDraw[] slots=bindings.get(submit.texture);
                if(slots==null){slots=new PreparedDraw[Pass.values().length*4];bindings.put(submit.texture,slots);}
                int slot=submit.bindingSlot();PreparedDraw prepared=slots[slot];
                if(prepared==null){prepared=new PreparedDraw(submit);slots[slot]=prepared;draws.add(prepared);}
                prepared.triangles.addAll(submit.triangles);
            }
            for(PreparedDraw prepared:draws) {
                if(sortQuads)drawSorted(prepared.triangles,true);
                else for(Triangle triangle:prepared.triangles)draw(triangle,prepared.binding.blend);
            }
        }
    }
    private float[] screen(Vertex v) {
        float iw=1f/v.a[3];
        return new float[]{(v.a[0]*iw+1)*width*0.5f,(1-v.a[1]*iw)*height*0.5f,v.a[2]*iw,iw};
    }
    private static float edge(float[] a,float[] b,float x,float y) { return (b[0]-a[0])*(y-a[1])-(b[1]-a[1])*(x-a[0]); }
    private static boolean topLeft(float[] a,float[] b) { return b[1]<a[1] || b[1]==a[1] && b[0]>a[0]; }
    private void draw(Triangle t,boolean transparent) {
        Vertex va=t.a,vb=t.b,vc=t.c;
        float[] a=screen(va),b=screen(vb),c=screen(vc);
        float area=edge(a,b,c[0],c[1]);
        if(Math.abs(area)<1e-8f) return;
        // Java's CCW front face has negative screen-space area after Y flips
        // downward. Capture it before normalizing raster traversal winding.
        boolean front=area<0;
        if(t.pass==Pass.JAVA_SOLID&&!front)return;
        int color=t.pass.perFaceLighting()&&(lighting!=null||entityShader!=null)&&!front?10:6;
        if(area<0) { float[] swap=b;b=c;c=swap; Vertex v=vb;vb=vc;vc=v;area=-area; }
        int x0=Math.max(0,(int)Math.floor(Math.min(a[0],Math.min(b[0],c[0]))));
        int x1=Math.min(width-1,(int)Math.ceil(Math.max(a[0],Math.max(b[0],c[0]))));
        int y0=Math.max(0,(int)Math.floor(Math.min(a[1],Math.min(b[1],c[1]))));
        int y1=Math.min(height-1,(int)Math.ceil(Math.max(a[1],Math.max(b[1],c[1]))));
        float[] glintSample=t.glint==null?null:new float[4];
        boolean entity=entityShader!=null&&t.pass!=Pass.GENERAL&&t.glint==null;
        float[] entityColor=entity?new float[4]:null,entityLight=entity?new float[4]:null,entityOverlay=entity?new float[4]:null;
        for(int y=y0;y<=y1;y++) for(int x=x0;x<=x1;x++) {
            float wa=edge(b,c,x+0.5f,y+0.5f),wb=edge(c,a,x+0.5f,y+0.5f),wc=edge(a,b,x+0.5f,y+0.5f);
            if(wa<0 || wa==0&&!topLeft(b,c) || wb<0 || wb==0&&!topLeft(c,a) || wc<0 || wc==0&&!topLeft(a,b)) continue;
            wa/=area;wb/=area;wc/=area;
            float z=wa*a[2]+wb*b[2]+wc*c[2];int index=y*width+x;
            float depthSlack=t.pass==Pass.GENERAL?0.000001f:0;
            if(z<0 || z>1 || z>depth[index]+depthSlack) continue;
            if(t.glint!=null&&z!=depth[index])continue;
            float divisor=wa*a[3]+wb*b[3]+wc*c[3];
            if(divisor<=0) continue;
            wa=wa*a[3]/divisor;wb=wb*b[3]/divisor;wc=wc*c[3]/divisor;
            if(t.glint!=null) {
                float u=wa*va.a[4]+wb*vb.a[4]+wc*vc.a[4],v=wa*va.a[5]+wb*vb.a[5]+wc*vc.a[5];
                JavaArmorGlint.sample(t.texture,u,v,glintSample);
                float spherical=wa*va.a[13]+wb*vb.a[13]+wc*vc.a[13],cylindrical=wa*va.a[14]+wb*vb.a[14]+wc*vc.a[14];
                pixels[index]=t.glint.fragment(glintSample,spherical,cylindrical,pixels[index]);
                continue; // EQUAL depth test, no depth write and no alpha replacement.
            }
            int sample=0xFFFFFFFF;
            if(t.texture!=null) {
                float u=wa*va.a[4]+wb*vb.a[4]+wc*vc.a[4],v=wa*va.a[5]+wb*vb.a[5]+wc*vc.a[5];
                int tx=texel(u,t.texture.getWidth(),t.pass!=Pass.GENERAL);
                int ty=texel(v,t.texture.getHeight(),t.pass!=Pass.GENERAL);
                sample=t.texture.getRGB(tx,ty);
            }
            // entity.fsh discards on texture alpha before multiplying vertex
            // colour/opacity. Fading an accepted texel must not turn cutout on.
            if((sample>>>24)/255f<t.pass.alphaCutout)continue;
            if(entity) {
                for(int channel=0;channel<3;channel++)
                    entityColor[channel]=wa*va.a[color+channel]+wb*vb.a[color+channel]+wc*vc.a[color+channel];
                entityColor[3]=wa*va.a[9]+wb*vb.a[9]+wc*vc.a[9];
                for(int channel=0;channel<4;channel++) {
                    entityLight[channel]=wa*va.a[15+channel]+wb*vb.a[15+channel]+wc*vc.a[15+channel];
                    entityOverlay[channel]=wa*va.a[19+channel]+wb*vb.a[19+channel]+wc*vc.a[19+channel];
                }
                float spherical=wa*va.a[13]+wb*vb.a[13]+wc*vc.a[13];
                float cylindrical=wa*va.a[14]+wb*vb.a[14]+wc*vc.a[14];
                entityShader.fragment(sample,entityColor,t.pass!=Pass.JAVA_WINGS,t.pass==Pass.JAVA_GLOW,
                    entityLight,spherical,cylindrical,entityOverlay);
                int alpha=t.pass.unblended()?255:channel(entityColor[3]*255);
                if(alpha!=0) {
                    int fragment=(alpha<<24)|(channel(entityColor[0]*255)<<16)
                        |(channel(entityColor[1]*255)<<8)|channel(entityColor[2]*255);
                    pixels[index]=t.pass.unblended()?fragment:over(fragment,pixels[index]);
                }
                // Neither ColorModulator nor the lightmap introduces an alpha
                // discard after the sampled-texture cutout has passed.
                if(t.pass.writesDepth())depth[index]=z;
                continue;
            }
            // The destination world is opaque. An unblended Java draw
            // overwrites its RGB regardless of sampled/tinted alpha; keep
            // opaque overlay coverage so composition preserves that overwrite.
            int alpha=t.pass.unblended()?255:channel((sample>>>24)*(wa*va.a[9]+wb*vb.a[9]+wc*vc.a[9]));
            if(alpha==0) {
                // Java's shader has no post-tint alpha discard. A surviving
                // zero-alpha base fragment still participates in depth writes.
                if(t.pass.writesDepth())depth[index]=z;
                continue;
            }
            int r=channel(((sample>>>16)&255)*(wa*va.a[color]+wb*vb.a[color]+wc*vc.a[color]));
            int g=channel(((sample>>>8)&255)*(wa*va.a[color+1]+wb*vb.a[color+1]+wc*vc.a[color+1]));
            int bl=channel((sample&255)*(wa*va.a[color+2]+wb*vb.a[color+2]+wc*vc.a[color+2]));
            int fragment=(alpha<<24)|(r<<16)|(g<<8)|bl;
            pixels[index]=t.pass.unblended()?fragment:over(fragment,pixels[index]);
            if(t.pass.writesDepth()||t.pass==Pass.GENERAL&&!transparent&&alpha==255)depth[index]=z;
        }
    }
    private static int texel(float coordinate,int size,boolean repeat) {
        if(!repeat)return Math.min(size-1,Math.max(0,(int)Math.floor(coordinate*size)));
        // DynamicTexture.createTexture selects REPEAT + NEAREST for Arctic's
        // Java cosmetic images. Reduce the floored texel before integer cast:
        // negative/tiny coordinates and large UVs must wrap, not saturate.
        int texel=(int)(Math.floor((double)coordinate*size)%size);
        return texel<0?texel+size:texel;
    }
    private static int channel(float f) { return Math.max(0,Math.min(255,Math.round(f))); }
    private static int over(int src,int dst) {
        int sa=src>>>24,da=dst>>>24,alpha=sa+(da*(255-sa)+127)/255;
        if(alpha==0) return 0;
        int out=alpha<<24;
        for(int shift=0;shift<=16;shift+=8) {
            int n=((src>>>shift)&255)*sa+(((dst>>>shift)&255)*da*(255-sa)+127)/255;
            out|=((n+alpha/2)/alpha)<<shift;
        }
        return out;
    }
}
