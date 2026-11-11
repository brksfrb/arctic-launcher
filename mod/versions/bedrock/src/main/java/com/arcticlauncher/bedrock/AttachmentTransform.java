package com.arcticlauncher.bedrock;

import java.util.Arrays;

/** Interpolate owned root-space affine palettes without blending rotations
 * into singular matrices. Native hierarchy scale can introduce shear, so a
 * rigid-only decomposition would discard valid attachment intervals. */
final class AttachmentTransform {
    private AttachmentTransform() {}
    private static final class Parts {
        final boolean reflected;final float[] rotation;final double[] stretch;
        Parts(boolean reflected,float[] rotation,double[] stretch) {
            this.reflected=reflected;this.rotation=rotation;this.stretch=stretch;
        }
    }
    static float[] between(float[] a,float[] b,double phase) {
        if(!Double.isFinite(phase)||phase<0||phase>1)throw new IllegalArgumentException("invalid attachment phase");
        if(phase==0||Arrays.equals(a,b))return a.clone();if(phase==1)return b.clone();
        Parts x=parts(a),y=parts(b);
        if(x.reflected!=y.reflected)throw new IllegalArgumentException("attachment handedness changed between samples");
        float[] rotation=WingSample.interpolateBasis(x.rotation,y.rotation,phase),out=new float[16];
        for(int r=0;r<3;r++)for(int c=0;c<3;c++) {
            double value=0;
            for(int k=0;k<3;k++)value+=(double)rotation[r*4+k]*(x.stretch[k*3+c]+(y.stretch[k*3+c]-x.stretch[k*3+c])*phase);
            // Both endpoints use the same fixed reflection. Interpolating
            // their positive stretch keeps its parity throughout the curve.
            out[c*4+r]=(float)(x.reflected&&c==0?-value:value);
        }
        for(int r=0;r<3;r++)out[12+r]=(float)(a[12+r]+((double)b[12+r]-a[12+r])*phase);
        out[15]=1;return out;
    }
    private static Parts parts(float[] matrix) {
        if(matrix==null||matrix.length!=16)throw new IllegalArgumentException("missing attachment transform");
        double[] m=new double[9];double scale=0;
        for(int r=0;r<3;r++)for(int c=0;c<3;c++) {
            double value=matrix[c*4+r];
            if(!Double.isFinite(value))throw new IllegalArgumentException("invalid attachment transform");
            m[r*3+c]=value;scale=Math.max(scale,Math.abs(value));
        }
        if(scale==0)throw new IllegalArgumentException("collapsed attachment basis has no interpolable rotation");
        double[] rotation=new double[9];for(int i=0;i<9;i++)rotation[i]=m[i]/scale;
        double determinant=det(rotation);
        if(!Double.isFinite(determinant)||Math.abs(determinant)<1e-15)
            throw new IllegalArgumentException("singular or ill-conditioned attachment basis");
        boolean reflected=determinant<0;
        if(reflected)for(int r=0;r<3;r++){m[r*3]=-m[r*3];rotation[r*3]=-rotation[r*3];}
        // Scaled Newton polar decomposition: R = polar(A), S = R^T A.
        // The balancing factor avoids long convergence for nonuniform scales.
        for(int iteration=0;iteration<32;iteration++) {
            double[] inverse=inverse(rotation),next=new double[9];
            double weight=Math.sqrt(norm(inverse)/norm(rotation)),change=0,largest=0;
            if(!Double.isFinite(weight)||weight==0)throw new IllegalArgumentException("unstable attachment polar factor");
            for(int r=0;r<3;r++)for(int c=0;c<3;c++) {
                int i=r*3+c;next[i]=.5*(weight*rotation[i]+inverse[c*3+r]/weight);
                change=Math.max(change,Math.abs(next[i]-rotation[i]));largest=Math.max(largest,Math.abs(next[i]));
            }
            rotation=next;if(change<=1e-10*Math.max(1,largest))break;
        }
        for(int a=0;a<3;a++)for(int b=0;b<3;b++) {
            double dot=0;for(int r=0;r<3;r++)dot+=rotation[r*3+a]*rotation[r*3+b];
            if(!Double.isFinite(dot)||Math.abs(dot-(a==b?1:0))>1e-7)
                throw new IllegalArgumentException("attachment polar rotation did not converge");
        }
        if(Math.abs(det(rotation)-1)>1e-7)throw new IllegalArgumentException("invalid attachment polar orientation");
        double[] stretch=new double[9];
        for(int r=0;r<3;r++)for(int c=0;c<3;c++)for(int k=0;k<3;k++)stretch[r*3+c]+=rotation[k*3+r]*m[k*3+c];
        for(int r=0;r<3;r++)for(int c=r+1;c<3;c++) {
            double value=(stretch[r*3+c]+stretch[c*3+r])*.5;stretch[r*3+c]=value;stretch[c*3+r]=value;
        }
        positive(stretch,scale);
        for(int r=0;r<3;r++)for(int c=0;c<3;c++) {
            double restored=0;for(int k=0;k<3;k++)restored+=rotation[r*3+k]*stretch[k*3+c];
            if(!Double.isFinite(restored)||Math.abs(restored-m[r*3+c])>scale*1e-6)
                throw new IllegalArgumentException("attachment polar reconstruction differs");
        }
        float[] basis=new float[12];for(int r=0;r<3;r++)for(int c=0;c<3;c++)basis[r*4+c]=(float)rotation[r*3+c];
        return new Parts(reflected,basis,stretch);
    }
    private static void positive(double[] matrix,double scale) {
        double[] lower=new double[9];
        for(int r=0;r<3;r++)for(int c=0;c<=r;c++) {
            double value=matrix[r*3+c]/scale;
            for(int k=0;k<c;k++)value-=lower[r*3+k]*lower[c*3+k];
            if(r==c) {
                if(!Double.isFinite(value)||value<=1e-15)throw new IllegalArgumentException("attachment stretch is not positive definite");
                lower[r*3+c]=Math.sqrt(value);
            } else lower[r*3+c]=value/lower[c*3+c];
        }
    }
    private static double norm(double[] matrix) {
        double value=0;for(double entry:matrix)value+=entry*entry;return Math.sqrt(value);
    }
    private static double det(double[] m) {
        return m[0]*(m[4]*m[8]-m[5]*m[7])-m[1]*(m[3]*m[8]-m[5]*m[6])+m[2]*(m[3]*m[7]-m[4]*m[6]);
    }
    private static double[] inverse(double[] m) {
        double d=det(m);
        if(!Double.isFinite(d)||d==0)throw new IllegalArgumentException("singular attachment polar factor");
        double[] result={m[4]*m[8]-m[5]*m[7],m[2]*m[7]-m[1]*m[8],m[1]*m[5]-m[2]*m[4],
            m[5]*m[6]-m[3]*m[8],m[0]*m[8]-m[2]*m[6],m[2]*m[3]-m[0]*m[5],
            m[3]*m[7]-m[4]*m[6],m[1]*m[6]-m[0]*m[7],m[0]*m[4]-m[1]*m[3]};
        for(int i=0;i<9;i++){result[i]/=d;if(!Double.isFinite(result[i]))throw new IllegalArgumentException("invalid attachment polar inverse");}
        return result;
    }
}
