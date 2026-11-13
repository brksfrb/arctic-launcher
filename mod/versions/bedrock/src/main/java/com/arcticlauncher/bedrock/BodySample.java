package com.arcticlauncher.bedrock;

import java.io.*;
import java.util.Arrays;
import java.util.Objects;

/** Owned native pose, model root, color, scene light, skin material and visibility;
 * no native addresses. Light is a discrete observed attribute. */
final class BodySample {
    private final float[][] parts;
    private final float[] color;
    private final Integer packedLight;
    private final JavaSkinMaterial skinMaterial;
    private final JavaSkinVisibility skinVisibility;
    private final JavaLayerVisibility layerVisibility;
    final ModelRootSample root;
    BodySample(float[][] values) {
        this(values,null);
    }
    BodySample(float[][] values,ModelRootSample root) {
        this(values,root,null);
    }
    BodySample(float[][] values,ModelRootSample root,float[] color) {
        this(values,root,color,null);
    }
    BodySample(float[][] values,ModelRootSample root,float[] color,Integer packedLight) {
        this(values,root,color,packedLight,null);
    }
    BodySample(float[][] values,ModelRootSample root,float[] color,Integer packedLight,JavaSkinMaterial skinMaterial) {
        this(values,root,color,packedLight,skinMaterial,null);
    }
    BodySample(float[][] values,ModelRootSample root,float[] color,Integer packedLight,JavaSkinMaterial skinMaterial,JavaSkinVisibility skinVisibility) {
        this(values,root,color,packedLight,skinMaterial,skinVisibility,null);
    }
    BodySample(float[][] values,ModelRootSample root,float[] color,Integer packedLight,JavaSkinMaterial skinMaterial,JavaSkinVisibility skinVisibility,JavaLayerVisibility layerVisibility) {
        if(values==null||values.length!=6)throw new IllegalArgumentException("recorded body needs six parts");
        parts=new float[6][];
        for(int i=0;i<6;i++) {WorldFrame.rigid(values[i]);parts[i]=values[i].clone();}
        this.root=root;
        WorldFrame.color(color);this.color=color==null?null:color.clone();
        this.packedLight=packedLight;
        this.skinMaterial=skinMaterial;
        this.skinVisibility=skinVisibility;
        this.layerVisibility=layerVisibility;
    }
    float[][] copyParts() {
        float[][] copy=new float[6][];for(int i=0;i<6;i++)copy[i]=parts[i].clone();return copy;
    }
    float[] copyColor(){return color==null?null:color.clone();}
    boolean hasColor(){return color!=null;}
    Integer packedLight(){return packedLight;}
    boolean hasPackedLight(){return packedLight!=null;}
    JavaSkinMaterial skinMaterial(){return skinMaterial;}
    boolean hasSkinMaterial(){return skinMaterial!=null;}
    JavaSkinVisibility skinVisibility(){return skinVisibility;}
    boolean hasSkinVisibility(){return skinVisibility!=null;}
    JavaLayerVisibility layerVisibility(){return layerVisibility;}
    boolean hasLayerVisibility(){return layerVisibility!=null&&layerVisibility.complete();}
    BodySample copy(){return new BodySample(parts,root,color,packedLight,skinMaterial,skinVisibility,layerVisibility);}
    boolean same(BodySample other) {return other!=null&&Arrays.deepEquals(parts,other.parts)&&Arrays.equals(color,other.color)
        &&Objects.equals(packedLight,other.packedLight)&&(skinMaterial==null?other.skinMaterial==null:skinMaterial.same(other.skinMaterial))
        &&(skinVisibility==null?other.skinVisibility==null:skinVisibility.same(other.skinVisibility))
        &&(layerVisibility==null?other.layerVisibility==null:layerVisibility.same(other.layerVisibility))
        &&(root==null?other.root==null:root.same(other.root));}
    void write(DataOutput output) throws IOException {for(float[] part:parts)for(float value:part)output.writeFloat(value);}
    void write(DataOutput output,boolean roots) throws IOException {
        write(output);if(roots){if(root==null)throw new IOException("recorded body is missing its model root");root.write(output);}
    }
    void write(DataOutput output,boolean roots,boolean colors) throws IOException {
        write(output,roots);
        if(colors){output.writeBoolean(color!=null);if(color!=null)for(float value:color)output.writeFloat(value);}
    }
    void write(DataOutput output,boolean roots,boolean colors,boolean lights) throws IOException {
        write(output,roots,colors);
        if(lights){output.writeBoolean(packedLight!=null);if(packedLight!=null)output.writeInt(packedLight);}
    }
    void write(DataOutput output,boolean roots,boolean colors,boolean lights,boolean skins) throws IOException {
        write(output,roots,colors,lights);if(skins)JavaSkinMaterial.write(output,skinMaterial);
    }
    void write(DataOutput output,boolean roots,boolean colors,boolean lights,boolean skins,boolean visibility) throws IOException {
        write(output,roots,colors,lights,skins);if(visibility)JavaSkinVisibility.write(output,skinVisibility);
    }
    void write(DataOutput output,boolean roots,boolean colors,boolean lights,boolean skins,boolean visibility,boolean layers) throws IOException {
        write(output,roots,colors,lights,skins,visibility);if(layers)JavaLayerVisibility.write(output,layerVisibility);
    }
    static BodySample read(DataInput input) throws IOException {
        return read(input,false,false,false,false,false);
    }
    static BodySample read(DataInput input,boolean roots) throws IOException {
        return read(input,roots,false,false,false,false);
    }
    static BodySample read(DataInput input,boolean roots,boolean colors) throws IOException {
        return read(input,roots,colors,false,false,false);
    }
    static BodySample read(DataInput input,boolean roots,boolean colors,boolean lights) throws IOException {
        return read(input,roots,colors,lights,false,false);
    }
    static BodySample read(DataInput input,boolean roots,boolean colors,boolean lights,boolean skins) throws IOException {
        return read(input,roots,colors,lights,skins,false);
    }
    static BodySample read(DataInput input,boolean roots,boolean colors,boolean lights,boolean skins,boolean visibility) throws IOException {
        return read(input,roots,colors,lights,skins,visibility,false);
    }
    static BodySample read(DataInput input,boolean roots,boolean colors,boolean lights,boolean skins,boolean visibility,boolean layers) throws IOException {
        float[][] parts=new float[6][12];for(float[] part:parts)for(int i=0;i<12;i++)part[i]=input.readFloat();
        ModelRootSample root=roots?ModelRootSample.read(input):null;
        float[] color=null;Integer light=null;
        if(colors) {
            int present=input.readUnsignedByte();if(present>1)throw new IOException("invalid recorded body color presence");
            if(present==1){color=new float[4];for(int i=0;i<4;i++)color[i]=input.readFloat();}
        }
        if(lights) {
            int present=input.readUnsignedByte();if(present>1)throw new IOException("invalid recorded body light presence");
            if(present==1)light=input.readInt();
        }
        return new BodySample(parts,root,color,light,skins?JavaSkinMaterial.read(input):null,visibility?JavaSkinVisibility.read(input):null,
            layers?JavaLayerVisibility.read(input):null);
    }
    static BodySample between(BodySample a,BodySample b,double phase) {
        if(!Double.isFinite(phase)||phase<0||phase>1)throw new IllegalArgumentException("invalid recorded body phase");
        if(phase==0)return a.copy();if(phase==1)return b.copy();
        if(a.same(b))return a.copy();
        float[][] result=new float[6][];
        for(int i=0;i<6;i++)result[i]=WingSample.interpolateRigid(a.parts[i],b.parts[i],phase);
        if((a.root==null)!=(b.root==null))throw new IllegalArgumentException("body root availability changed between samples");
        float[] color=null;
        if(a.color!=null&&b.color!=null){color=new float[4];for(int i=0;i<4;i++)color[i]=Float.floatToIntBits(a.color[i])==Float.floatToIntBits(b.color[i])
            ?a.color[i]:(float)(a.color[i]+((double)b.color[i]-a.color[i])*phase);}
        // Packed block/sky coordinates are discrete attributes, not a scalar
        // animation curve. A changing or missing endpoint gives no evidence
        // of the transition time between these two observations.
        Integer light=Objects.equals(a.packedLight,b.packedLight)?a.packedLight:null;
        // Overlay texels are selected discrete shader inputs. A changing
        // selection does not imply a continuous overlay color curve.
        JavaSkinMaterial material=a.skinMaterial!=null&&a.skinMaterial.same(b.skinMaterial)?a.skinMaterial:null;
        JavaSkinVisibility visibility=a.skinVisibility!=null&&a.skinVisibility.same(b.skinVisibility)?a.skinVisibility:null;
        // Discrete eligibility changes have no inferred transition timestamp.
        JavaLayerVisibility layers=JavaLayerVisibility.between(a.layerVisibility,b.layerVisibility);
        return new BodySample(result,a.root==null?null:ModelRootSample.between(a.root,b.root,phase),color,light,material,visibility,layers);
    }
}
