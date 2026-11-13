package com.arcticlauncher.bedrock;

/** JOML 1.10.8 Matrix3f.normal operation order for a final row-major native
 * affine pose. This does not recover PoseStack's separate trusted-normal history.
 * All arithmetic follows the default non-FMA path. */
final class JavaNormalMatrix {
    private JavaNormalMatrix() {}

    static float[] fromPose(float[] pose) {
        float a=pose[0],b=pose[1],c=pose[2];
        float d=pose[4],e=pose[5],f=pose[6];
        float g=pose[8],h=pose[9],i=pose[10];
        // JOML names matrix entries by column then row. Preserve its six
        // intermediate products and determinant grouping before taking 1/det.
        float ae=a*e,db=d*b,gb=g*b,ah=a*h,dh=d*h,ge=g*e;
        float determinant=((ae-db)*i+(gb-ah)*f)+(dh-ge)*c;
        float inverse=1f/determinant;
        return new float[]{
            (e*i-f*h)*inverse,(f*g-d*i)*inverse,(dh-ge)*inverse,
            (c*h-b*i)*inverse,(a*i-c*g)*inverse,(gb-ah)*inverse,
            (b*f-c*e)*inverse,(c*d-a*f)*inverse,(ae-db)*inverse
        };
    }

    static void transform(float[] matrix,float nx,float ny,float nz,com.arcticlauncher.client.looks.Xform out) {
        // Matrix3f.transform expands nested Math.fma calls; the default JOML
        // fallback evaluates a*b+(c*d+e*f), not a left-associated sum.
        float x=matrix[0]*nx+(matrix[1]*ny+matrix[2]*nz);
        float y=matrix[3]*nx+(matrix[4]*ny+matrix[5]*nz);
        float z=matrix[6]*nx+(matrix[7]*ny+matrix[8]*nz);
        float inverse=1f/(float)Math.sqrt(x*x+(y*y+z*z));
        out.x=x*inverse;out.y=y*inverse;out.z=z*inverse;
        // Deliberately preserve singular/nonfinite results through this stage.
        // BufferBuilder's later signed-byte conversion maps NaN to zero; a
        // fabricated -Z normal would shade collapsed surfaces differently.
    }
}
