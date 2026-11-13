package com.arcticlauncher.bedrock;

import com.google.gson.JsonArray;
import java.io.*;
import java.util.Arrays;

/** Copied actual model basis in blocks and a double world-space origin. The
 * camera-relative float offset is added to its observed origin before storage;
 * playback subtracts the current camera before narrowing the offset again. */
final class ModelRootSample {
    private final float[] basis;private final double[] position;
    ModelRootSample(float[] basis,double[] position) {
        if(basis==null||basis.length!=9||position==null||position.length!=3)throw new IllegalArgumentException("invalid recorded model root");
        for(float value:basis)if(!Float.isFinite(value)||Math.abs(value)>32_000_000)throw new IllegalArgumentException("invalid recorded model basis");
        for(double value:position)if(!Double.isFinite(value)||Math.abs(value)>64_000_000)throw new IllegalArgumentException("invalid recorded model position");
        this.basis=basis.clone();this.position=position.clone();
    }
    static ModelRootSample observe(WorldFrame world,WorldFrame.Player player) {
        if(!Boolean.TRUE.equals(world.cameraRelative))throw new IllegalArgumentException("native model-root capture needs declared camera-relative coordinates");
        if(!"native-model-draw".equals(player.modelSource)||player.model==null||player.model.length!=12
                ||world.camera==null||world.camera.length!=6||player.nativeGlobal==null
                ||!player.id.equals(player.nativeGlobal.get("actorId").getAsString()))throw new IllegalArgumentException("model root has no owned native draw");
        JsonArray nativeMatrix=player.nativeGlobal.getAsJsonObject("draw").getAsJsonArray("matrix");
        if(nativeMatrix.size()!=16)throw new IllegalArgumentException("invalid native root matrix");
        float[] basis=new float[9];double[] position=new double[3];
        for(int row=0;row<3;row++) {
            for(int column=0;column<4;column++) {
                float expected=nativeMatrix.get(column*4+row).getAsFloat()*(column==3?1f:16f);
                if(!Float.isFinite(expected)||Float.floatToIntBits(expected)!=Float.floatToIntBits(player.model[row*4+column]))
                    throw new IllegalArgumentException("model root differs from its native pixel draw");
                if(column<3)basis[row*3+column]=expected;
            }
            if(!Double.isFinite(world.camera[row])||Math.abs(world.camera[row])>32_000_000)throw new IllegalArgumentException("invalid observed camera origin");
            // The native origin is f32. Round its shortest JSON spelling back
            // through float before widening, just as for the native tick phase.
            position[row]=(double)(float)world.camera[row]+(double)player.model[row*4+3];
        }
        return new ModelRootSample(basis,position);
    }
    float[] relative(double[] camera) {
        if(camera==null||camera.length<3)throw new IllegalArgumentException("missing replay camera origin");
        float[] model=new float[12];
        for(int row=0;row<3;row++) {
            if(!Double.isFinite(camera[row])||Math.abs(camera[row])>32_000_000)throw new IllegalArgumentException("invalid replay camera origin");
            double offset=position[row]-(double)(float)camera[row];
            if(!Double.isFinite(offset)||Math.abs(offset)>32_000_000)throw new IllegalArgumentException("recorded actor is outside the relative model range");
            System.arraycopy(basis,row*3,model,row*4,3);model[row*4+3]=(float)offset;
        }
        return model;
    }
    boolean same(ModelRootSample other) {return other!=null&&Arrays.equals(basis,other.basis)&&Arrays.equals(position,other.position);}
    void write(DataOutput out) throws IOException {for(float value:basis)out.writeFloat(value);for(double value:position)out.writeDouble(value);}
    static ModelRootSample read(DataInput in) throws IOException {
        float[] basis=new float[9];double[] position=new double[3];
        for(int i=0;i<9;i++)basis[i]=in.readFloat();for(int i=0;i<3;i++)position[i]=in.readDouble();return new ModelRootSample(basis,position);
    }
    static ModelRootSample between(ModelRootSample a,ModelRootSample b,double phase) {
        if(!Double.isFinite(phase)||phase<0||phase>1)throw new IllegalArgumentException("invalid model root phase");
        if(phase==0)return a;if(phase==1)return b;
        // This immutable root is already valid at both endpoints. A constant
        // affine basis needs no interpolation/decomposition or camera fallback.
        if(a.same(b))return a;
        float[] basis=blendBasis(a.basis,b.basis,phase);double[] position=new double[3];
        for(int row=0;row<3;row++) {
            position[row]=Double.doubleToLongBits(a.position[row])==Double.doubleToLongBits(b.position[row])
                ?a.position[row]:a.position[row]+(b.position[row]-a.position[row])*phase;
        }
        return new ModelRootSample(basis,position);
    }
    private static float[] blendBasis(float[] a,float[] b,double phase) {
        // Native roots are affine. An unchanged valid basis remains exact even
        // when the actor's separate double world-space position changes.
        if(Arrays.equals(a,b))return a.clone();
        float[] left=new float[12],right=new float[12],result=new float[9];
        for(int row=0;row<3;row++){System.arraycopy(a,row*3,left,row*4,3);System.arraycopy(b,row*3,right,row*4,3);}
        try {
            // Retain the existing arithmetic for changing positive-handed,
            // orthogonal bases accepted by the previous archive sampler.
            float[] interpolated=WingSample.interpolateBasis(left,right,phase);
            for(int row=0;row<3;row++)System.arraycopy(interpolated,row*4,result,row*3,3);
        } catch(IllegalArgumentException affineBasis) {
            // Reuse the attachment palette's bounded polar interpolation for
            // reflection/shear. It rejects parity changes, singular factors,
            // failed convergence and non-positive stretch.
            float[] leftAffine=new float[16],rightAffine=new float[16];leftAffine[15]=rightAffine[15]=1;
            for(int row=0;row<3;row++)for(int column=0;column<3;column++) {
                leftAffine[column*4+row]=a[row*3+column];rightAffine[column*4+row]=b[row*3+column];
            }
            float[] interpolated=AttachmentTransform.between(leftAffine,rightAffine,phase);
            for(int row=0;row<3;row++)for(int column=0;column<3;column++)result[row*3+column]=interpolated[column*4+row];
        }
        return result;
    }
}
