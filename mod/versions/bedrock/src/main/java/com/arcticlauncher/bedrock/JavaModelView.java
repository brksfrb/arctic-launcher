package com.arcticlauncher.bedrock;

/** This renderer's submitted root under its checked column-major affine view.
 * This is neither an installed Java pose receipt nor native GPU ownership. */
final class JavaModelView {
    private JavaModelView() {}
    static float[] translation(float[] view,float[] root) {
        if(view==null)return null;
        if(view.length!=16||root==null||root.length!=12)throw new IllegalArgumentException("invalid submitted model/view matrix");
        for(float value:view)if(!Float.isFinite(value))throw new IllegalArgumentException("non-finite submitted view");
        for(float value:root)if(!Float.isFinite(value))throw new IllegalArgumentException("non-finite submitted model");
        float[] position=new float[3];
        for(int row=0;row<3;row++) {
            // JOML 1.10.8 mulAffine's translation column expands three
            // nested Math.fma calls. Preserve its default non-fused grouping.
            position[row]=view[row]*root[3]+(view[4+row]*root[7]+(view[8+row]*root[11]+view[12+row]));
            if(!Float.isFinite(position[row]))return null;
        }
        return position;
    }
}
