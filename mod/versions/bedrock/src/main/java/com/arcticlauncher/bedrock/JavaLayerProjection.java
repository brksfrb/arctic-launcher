package com.arcticlauncher.bedrock;

/** Java 26.2 VIEW_OFFSET_Z_LAYERING before projection, evaluated from an
 * explicit native view/projection pair. No camera is reconstructed from FOV
 * or the already-combined matrix. Matrices are column-major. */
final class JavaLayerProjection {
    private JavaLayerProjection() {}

    static float[] combine(float[] view,float[] projection,String type,boolean reverseZ,boolean equipment) {
        float[] actualView=view.clone(),clip=projection.clone();
        if(equipment) {
            if("perspective".equals(type)) {
                // ProjectionType.lambda$static$0: Matrix4f.scale(1 - 1/4096).
                // Postmultiplication scales the first three columns only.
                float scale=1f-1f/4096f;
                for(int i=0;i<12;i++)actualView[i]*=scale;
            } else if("orthographic".equals(type)) {
                // ProjectionType.lambda$static$1: translate(0,0,1/512).
                // JOML's default non-FMA translateGeneric groups rightward.
                for(int row=0;row<4;row++)actualView[12+row]=view[row]*0f
                    +(view[4+row]*0f+(view[8+row]*(1f/512f)+view[12+row]));
            } else throw new IllegalArgumentException("unknown equipment projection type");
        }
        // Match live_frame's existing OpenGL -> DirectX clip-row conversion
        // before multiplying by the view, including off-axis coefficients.
        for(int column=0;column<4;column++)clip[column*4+2]=(projection[column*4+2]+projection[column*4+3])*.5f;
        float[] result=new float[16];
        for(int column=0;column<4;column++)for(int row=0;row<4;row++) {
            float sum=0;
            for(int k=0;k<4;k++)sum+=clip[k*4+row]*actualView[column*4+k];
            result[column*4+row]=sum;
        }
        // The software depth buffer is conventional near=0/far=1 even when
        // the observed native depth surface uses reverse Z.
        if(reverseZ)for(int column=0;column<4;column++)result[column*4+2]=result[column*4+3]-result[column*4+2];
        return result;
    }
}
