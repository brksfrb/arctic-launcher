package com.arcticlauncher.bedrock;

/** Source port of the default Java projection's upload-sort function. The
 * inputs are the submitted positions of quad vertices 0 and 2, before clip
 * projection. This does not establish native/Java vertex-space equivalence. */
final class JavaQuadSorting {
    private final boolean orthographic;
    private JavaQuadSorting(boolean orthographic){this.orthographic=orthographic;}

    static JavaQuadSorting forProjection(String type) {
        if("perspective".equals(type))return new JavaQuadSorting(false);
        if("orthographic".equals(type))return new JavaQuadSorting(true);
        return null;
    }

    /** Null is unknown; an overflowing distance may legitimately be infinity.
     * Keep float rounding, signed zero and the default non-fused JOML grouping. */
    Float key(float[] diagonal) {
        if(diagonal==null||diagonal.length!=6)return null;
        for(float value:diagonal)if(!Float.isFinite(value))return null;
        float x=(diagonal[0]+diagonal[3])/2f;
        float y=(diagonal[1]+diagonal[4])/2f;
        float z=(diagonal[2]+diagonal[5])/2f;
        if(orthographic)return -z;
        // DISTANCE_TO_ORIGIN captures the zero vector as the distance receiver.
        float dx=0f-x,dy=0f-y,dz=0f-z;
        return dx*dx+(dy*dy+dz*dz);
    }
}
