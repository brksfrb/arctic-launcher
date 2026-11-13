package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.Xform;

/** Identical point/normal transforms for software and native mesh staging. */
abstract class AvatarPoseSink extends Xform {
    private final float[] pose,javaNormal;
    private final boolean javaPositions;

    AvatarPoseSink(float[] input,boolean javaNormals) {
        this(input,javaNormals,javaNormals);
    }
    AvatarPoseSink(float[] input,boolean javaNormals,boolean javaPositions) {
        if(input==null||input.length!=12)throw new IllegalArgumentException("expected a 3x4 pose");
        pose=input.clone();
        for(float value:pose)if(!Float.isFinite(value))throw new IllegalArgumentException("invalid pose");
        javaNormal=javaNormals?JavaNormalMatrix.fromPose(pose):null;
        this.javaPositions=javaPositions;
    }

    @Override public final void position(float px,float py,float pz) {
        if(javaPositions) {
            x=JavaAffinePose.point(pose,0,px,py,pz);
            y=JavaAffinePose.point(pose,1,px,py,pz);
            z=JavaAffinePose.point(pose,2,px,py,pz);
            return;
        }
        x=pose[0]*px+pose[1]*py+pose[2]*pz+pose[3];
        y=pose[4]*px+pose[5]*py+pose[6]*pz+pose[7];
        z=pose[8]*px+pose[9]*py+pose[10]*pz+pose[11];
    }

    @Override public final void normal(float nx,float ny,float nz) {
        if(javaNormal!=null){JavaNormalMatrix.transform(javaNormal,nx,ny,nz,this);return;}
        float a=pose[0],b=pose[1],c=pose[2],d=pose[4],e=pose[5],f=pose[6],g=pose[8],h=pose[9],i=pose[10];
        float det=a*(e*i-f*h)-b*(d*i-f*g)+c*(d*h-e*g);
        if(Math.abs(det)<1e-9f){x=0;y=0;z=-1;return;}
        x=((e*i-f*h)*nx+(f*g-d*i)*ny+(d*h-e*g)*nz)/det;
        y=((c*h-b*i)*nx+(a*i-c*g)*ny+(b*g-a*h)*nz)/det;
        z=((b*f-c*e)*nx+(c*d-a*f)*ny+(a*e-b*d)*nz)/det;
        float len=(float)Math.sqrt(x*x+y*y+z*z);
        if(len>0){x/=len;y/=len;z/=len;}
    }
}
