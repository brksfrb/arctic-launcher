package com.arcticlauncher.bedrock;

/** JOML 1.10.8 generic affine arithmetic in the default non-fused profile.
 * Matrices are row-major 3x4. Copied native matrices do not retain JOML's
 * original property flags; identity/translation shortcuts are not inferred. */
final class JavaAffinePose {
    private JavaAffinePose() {}

    static float point(float[] pose,int row,float x,float y,float z) {
        int i=row*4;
        return pose[i]*x+(pose[i+1]*y+(pose[i+2]*z+pose[i+3]));
    }
    static float[] multiply(float[] a,float[] b) {
        float[] result=new float[12];
        for(int row=0;row<3;row++) {
            int i=row*4;
            for(int col=0;col<3;col++)
                result[i+col]=a[i]*b[col]+(a[i+1]*b[4+col]+a[i+2]*b[8+col]);
            result[i+3]=point(a,row,b[3],b[7],b[11]);
        }
        return result;
    }
    static float[] translate(float[] pose,float x,float y,float z) {
        float[] translated=pose.clone();
        for(int row=0;row<3;row++)translated[row*4+3]=point(pose,row,x,y,z);
        return translated;
    }
}
