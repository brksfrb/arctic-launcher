package com.arcticlauncher.bedrock;

/** PlayerCapeModel.setupAnim -> ModelPart.rotateBy -> translateAndRotate,
 * reconstructed from Java 26.2/JOML 1.10.8 class data. Uses JOML's default
 * non-fast-math float sequence; native parent pose history is not recovered. */
final class JavaCapePose {
    private static final float PI=(float)Math.PI,HALF_PI=(float)(Math.PI*.5),TWO_PI=(float)(Math.PI*2);
    private static final float DEG=(float)(Math.PI/180);

    private JavaCapePose() {}

    /** Row-major 3x4 cape-part pose in blocks, before its body parent. */
    static float[] local(CapePhysics.Motion motion) {
        float[] angles=CapePhysics.angles(motion);
        Quaternion turn=new Quaternion();
        turn.rotateY(-PI);
        turn.rotateX(angles[0]*DEG);
        turn.rotateZ(angles[1]*DEG);
        turn.rotateY(angles[2]*DEG);

        // Keep both half-turns and the intermediate float operations. Their
        // mathematical cancellation cannot replace Java's rounded sequence.
        float[] baked=rotationZYX(0,PI,0),delta=turn.matrix(true),rotated=new float[9];
        for(int row=0;row<3;row++)for(int col=0;col<3;col++)
            rotated[row*3+col]=(baked[row*3]*delta[col]+baked[row*3+1]*delta[3+col])+baked[row*3+2]*delta[6+col];

        // Matrix3f.getEulerAnglesZYX, with float sqrt before atan2. Preserve
        // its domain behavior rather than clamping the matrix into range.
        float x=(float)Math.atan2(rotated[7],rotated[8]);
        float y=(float)Math.atan2(-rotated[6],(float)Math.sqrt(1-rotated[6]*rotated[6]));
        float z=(float)Math.atan2(rotated[3],rotated[0]);
        return part(x,y,z,0,0,2);
    }

    /** ModelPart.translateAndRotate's quaternion rotationZYX, with its part
     * translation in pixels. Shared by the independently posed vanilla wings. */
    static float[] part(float x,float y,float z,float px,float py,float pz) {
        Quaternion posed=new Quaternion();posed.rotationZYX(z,y,x);
        float[] m=posed.matrix(false);
        return new float[]{m[0],m[1],m[2],px/16,m[3],m[4],m[5],py/16,m[6],m[7],m[8],pz/16};
    }

    private static float sin(float angle){return (float)Math.sin(angle);}
    private static float cosFromSin(float sine,float angle) {
        float cosine=(float)Math.sqrt(1-sine*sine);
        float shifted=angle+HALF_PI;
        float wrapped=shifted-(int)(shifted/TWO_PI)*TWO_PI;
        if(wrapped<0)wrapped=TWO_PI+wrapped;
        return wrapped>=PI?-cosine:cosine;
    }

    /** JOML Matrix3f.rotationZYX, expressed in row-major order. */
    private static float[] rotationZYX(float z,float y,float x) {
        float sx=sin(x),cx=cosFromSin(sx,x),sy=sin(y),cy=cosFromSin(sy,y),sz=sin(z),cz=cosFromSin(sz,z);
        float nsz=-sz,nsy=-sy,nsx=-sx,czsy=cz*sy,szsy=sz*sy;
        return new float[]{
            cz*cy,nsz*cx+czsy*sx,nsz*nsx+czsy*cx,
            sz*cy,cz*cx+szsy*sx,cz*nsx+szsy*cx,
            nsy,cy*sx,cy*cx
        };
    }

    private static final class Quaternion {
        private float x,y,z,w=1;
        private void set(float x,float y,float z,float w){this.x=x;this.y=y;this.z=z;this.w=w;}
        void rotateX(float angle) {
            float s=sin(angle*.5f),c=cosFromSin(s,angle*.5f);
            set(w*s+x*c,y*c+z*s,z*c-y*s,w*c-x*s);
        }
        void rotateY(float angle) {
            float s=sin(angle*.5f),c=cosFromSin(s,angle*.5f);
            set(x*c-z*s,w*s+y*c,x*s+z*c,w*c-y*s);
        }
        void rotateZ(float angle) {
            float s=sin(angle*.5f),c=cosFromSin(s,angle*.5f);
            set(x*c+y*s,y*c-x*s,w*s+z*c,w*c-z*s);
        }
        void rotationZYX(float zAngle,float yAngle,float xAngle) {
            float sx=sin(xAngle*.5f),cx=cosFromSin(sx,xAngle*.5f);
            float sy=sin(yAngle*.5f),cy=cosFromSin(sy,yAngle*.5f);
            float sz=sin(zAngle*.5f),cz=cosFromSin(sz,zAngle*.5f);
            float cycz=cy*cz,sysz=sy*sz,sycz=sy*cz,cysz=cy*sz;
            set(sx*cycz-cx*sysz,cx*sycz+sx*cysz,cx*cysz-sx*sycz,cx*cycz+sx*sysz);
        }
        /** Matrix3f.rotate and Matrix4f.rotation/rotateAffine use distinct
         * subtraction/addition order for m10, retained here for signed zero. */
        float[] matrix(boolean matrix3Rotate) {
            float ww=w*w,xx=x*x,yy=y*y,zz=z*z;
            float zw=z*w,zw2=zw+zw,xy=x*y,xy2=xy+xy,xz=x*z,xz2=xz+xz;
            float yw=y*w,yw2=yw+yw,yz=y*z,yz2=yz+yz,xw=x*w,xw2=xw+xw;
            return new float[]{
                ((ww+xx)-zz)-yy,matrix3Rotate?xy2-zw2:-zw2+xy2,yw2+xz2,
                xy2+zw2,((yy-zz)+ww)-xx,yz2-xw2,
                xz2-yw2,yz2+xw2,((zz-yy)-xx)+ww
            };
        }
    }
}
