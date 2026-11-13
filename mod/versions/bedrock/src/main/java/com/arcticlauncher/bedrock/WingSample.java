package com.arcticlauncher.bedrock;

import java.io.*;
import java.util.Arrays;

/** Owned sampled geometry. Interpolation preserves positive scale and rotation,
 * rather than linearly blending matrix entries into a sheared wing. */
final class WingSample {
    final float[][] wings;
    final float[] body;
    WingSample(float[][] wings,float[] body) {
        WorldFrame.wings(wings);WorldFrame.rigid(body);
        if(wings==null)throw new IllegalArgumentException("recorded wings are missing");
        this.wings=new float[][]{wings[0].clone(),wings[1].clone()};this.body=body.clone();
    }
    boolean same(WingSample other) {
        return other!=null&&Arrays.equals(body,other.body)&&Arrays.deepEquals(wings,other.wings);
    }
    void write(DataOutput out) throws IOException {
        for(float value:body)out.writeFloat(value);
        for(float[] wing:wings)for(float value:wing)out.writeFloat(value);
    }
    static WingSample read(DataInput in) throws IOException {
        float[] body=new float[12];float[][] wings=new float[2][12];
        for(int i=0;i<12;i++)body[i]=in.readFloat();
        for(float[] wing:wings)for(int i=0;i<12;i++)wing[i]=in.readFloat();
        return new WingSample(wings,body);
    }
    static WingSample between(WingSample a,WingSample b,double t) {
        if(!Double.isFinite(t)||t<0||t>1)throw new IllegalArgumentException("invalid recorded wing phase");
        if(t==0)return new WingSample(a.wings,a.body);
        if(t==1)return new WingSample(b.wings,b.body);
        if(a.same(b))return new WingSample(a.wings,a.body);
        return new WingSample(new float[][]{blend(a.wings[0],b.wings[0],t),blend(a.wings[1],b.wings[1],t)},blend(a.body,b.body,t));
    }
    static float[] interpolateRigid(float[] a,float[] b,double t) {
        WorldFrame.rigid(a);WorldFrame.rigid(b);
        if(!Double.isFinite(t)||t<0||t>1)throw new IllegalArgumentException("invalid recorded body phase");
        if(t==0)return a.clone();if(t==1)return b.clone();
        float[] result=blend(a,b,t);WorldFrame.rigid(result);return result;
    }
    /** Orthogonal, positive-handed model bases, with no translation/unit
     * assumption. Quaternion interpolation avoids collapsing a turning actor.
     * Sheared, reflected or degenerate intervals remain unsupported gaps. */
    static float[] interpolateBasis(float[] a,float[] b,double t) {
        if(!Double.isFinite(t)||t<0||t>1)throw new IllegalArgumentException("invalid model basis phase");
        basis(a);basis(b);float[] result=blend(a,b,t);basis(result);return result;
    }
    private static void basis(float[] matrix) {
        if(matrix==null||matrix.length!=12)throw new IllegalArgumentException("missing model basis");
        for(float value:matrix)if(!Float.isFinite(value)||Math.abs(value)>32_000_000)throw new IllegalArgumentException("invalid model basis");
        double[] s=scale(matrix);
        for(double value:s)if(!Double.isFinite(value)||value<1e-9)throw new IllegalArgumentException("degenerate model basis");
        for(int a=0;a<3;a++)for(int b=a+1;b<3;b++) {
            double dot=0;for(int r=0;r<3;r++)dot+=(double)matrix[r*4+a]*matrix[r*4+b];
            if(Math.abs(dot)>.001*s[a]*s[b])throw new IllegalArgumentException("sheared model basis");
        }
        double det=(double)matrix[0]*(matrix[5]*(double)matrix[10]-matrix[6]*(double)matrix[9])
            -(double)matrix[1]*(matrix[4]*(double)matrix[10]-matrix[6]*(double)matrix[8])
            +(double)matrix[2]*(matrix[4]*(double)matrix[9]-matrix[5]*(double)matrix[8]);
        if(Math.abs(det/(s[0]*s[1]*s[2])-1)>.001)throw new IllegalArgumentException("reflected model basis");
    }
    private static float[] blend(float[] a,float[] b,double t) {
        // All callers supply already checked samples/bases. Preserve an exact
        // observed constant matrix, including its floating-point residuals,
        // instead of projecting it through quaternion decomposition again.
        if(Arrays.equals(a,b))return a.clone();
        double[] sa=scale(a),sb=scale(b),qa=quaternion(a,sa),qb=quaternion(b,sb);
        double dot=0;for(int i=0;i<4;i++)dot+=qa[i]*qb[i];
        if(dot<0) {dot=-dot;for(int i=0;i<4;i++)qb[i]=-qb[i];}
        dot=Math.min(1,dot);double wa=1-t,wb=t;
        if(dot<.9995) {double angle=Math.acos(dot),sine=Math.sin(angle);wa=Math.sin((1-t)*angle)/sine;wb=Math.sin(t*angle)/sine;}
        double[] q=new double[4];double norm=0;
        for(int i=0;i<4;i++) {q[i]=qa[i]*wa+qb[i]*wb;norm+=q[i]*q[i];}
        norm=Math.sqrt(norm);for(int i=0;i<4;i++)q[i]/=norm;
        double x=q[0],y=q[1],z=q[2],w=q[3];
        double[] rotation={1-2*(y*y+z*z),2*(x*y-z*w),2*(x*z+y*w),
            2*(x*y+z*w),1-2*(x*x+z*z),2*(y*z-x*w),
            2*(x*z-y*w),2*(y*z+x*w),1-2*(x*x+y*y)};
        float[] result=new float[12];
        for(int r=0;r<3;r++) {
            for(int c=0;c<3;c++)result[r*4+c]=(float)(rotation[r*3+c]*(sa[c]+(sb[c]-sa[c])*t));
            result[r*4+3]=(float)(a[r*4+3]+((double)b[r*4+3]-a[r*4+3])*t);
        }
        return result;
    }
    private static double[] scale(float[] m) {
        double[] result=new double[3];
        for(int c=0;c<3;c++) {for(int r=0;r<3;r++)result[c]+=(double)m[r*4+c]*m[r*4+c];result[c]=Math.sqrt(result[c]);}
        return result;
    }
    private static double[] quaternion(float[] m,double[] scale) {
        double[][] r=new double[3][3];for(int i=0;i<3;i++)for(int j=0;j<3;j++)r[i][j]=m[i*4+j]/scale[j];
        double[] q=new double[4];double trace=r[0][0]+r[1][1]+r[2][2];
        if(trace>0) {
            double s=Math.sqrt(trace+1)*2;q[3]=s/4;q[0]=(r[2][1]-r[1][2])/s;q[1]=(r[0][2]-r[2][0])/s;q[2]=(r[1][0]-r[0][1])/s;
        } else {
            int i=r[0][0]>r[1][1]?(r[0][0]>r[2][2]?0:2):(r[1][1]>r[2][2]?1:2),j=(i+1)%3,k=(i+2)%3;
            double s=Math.sqrt(Math.max(0,1+r[i][i]-r[j][j]-r[k][k]))*2;
            q[i]=s/4;q[j]=(r[j][i]+r[i][j])/s;q[k]=(r[k][i]+r[i][k])/s;q[3]=(r[k][j]-r[j][k])/s;
        }
        double norm=0;for(double v:q)norm+=v*v;norm=Math.sqrt(norm);
        if(!Double.isFinite(norm)||norm<.5)throw new IllegalArgumentException("invalid recorded wing rotation");
        for(int i=0;i<4;i++)q[i]/=norm;return q;
    }
}
