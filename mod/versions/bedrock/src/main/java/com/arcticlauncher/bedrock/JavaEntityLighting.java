package com.arcticlauncher.bedrock;

/** Java 26.2 Lighting.updateLevel + entity vertex lighting. Directional
 * shading is independent of the still-unported world lightmap/fog inputs. */
final class JavaEntityLighting {
    private static final JavaEntityLighting DEFAULT=new JavaEntityLighting(.2f,1,-.7f,-.2f,1,.7f);
    private static final JavaEntityLighting NETHER=new JavaEntityLighting(.2f,1,-.7f,-.2f,-1,.7f);
    private final float[] a,b;
    private JavaEntityLighting(float ax,float ay,float az,float bx,float by,float bz) {
        a=unit(ax,ay,az);b=unit(bx,by,bz);
    }
    private static float[] unit(float x,float y,float z) {
        // JOML 1.10.8's default non-FMA normalize/invsqrt operation order.
        float squared=x*x+(y*y+z*z);
        float inverse=1f/(float)Math.sqrt(squared);
        return new float[]{x*inverse,y*inverse,z*inverse};
    }
    static JavaEntityLighting world(String dimension,Boolean cameraRelative) {
        // An unspecified adapter may emit view-space normals. Require its
        // coordinate declaration before using fixed world-axis light vectors.
        if(cameraRelative==null||dimension==null)return null;
        switch(dimension) {
            case "overworld":case "the_end":return DEFAULT;
            case "the_nether":return NETHER;
            default:return null;
        }
    }
    float shade(float nx,float ny,float nz,boolean front) {
        float x=a[0]*nx+a[1]*ny+a[2]*nz,y=b[0]*nx+b[1]*ny+b[2]*nz;
        if(!front){x=-x;y=-y;}
        return Math.min(1f,(Math.max(0,x)+Math.max(0,y))*.6f+.4f);
    }
    /** BufferBuilder.normalIntValue truncates clamp(n,-1,1)*127 to a byte.
     * DefaultVertexFormat feeds it to entity.vsh as RGBA8_SNORM. Do not
     * renormalize the decoded components: the shader uses those values. */
    static float shaderNormal(float value) {
        return (byte)(int)(Math.max(-1f,Math.min(1f,value))*127f)/127f;
    }
}
