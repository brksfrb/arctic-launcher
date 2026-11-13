package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.Xform;

/** ModelPart.Cube/Polygon face layout, including mirroring and the reversed
 * max-Y face V. Dimensions/pivots are supplied by the owning model; this class
 * does not infer a native bone's coordinate space or animation. */
final class JavaModelBox {
    // DOWN, UP, WEST, NORTH, EAST, SOUTH.
    private static final int[][] FACES={
        {5,4,0,1},{2,3,7,6},{0,4,7,3},
        {1,0,3,2},{5,1,2,6},{4,5,6,7}
    };
    private static final float[][] NORMALS={
        {0,-1,0},{0,1,0},{-1,0,0},{0,0,-1},{1,0,0},{0,0,1}
    };
    private final float[][] points,uv;
    private final int textureWidth,textureHeight;
    private final boolean mirror;

    JavaModelBox(float x,float y,float z,float width,float height,float depth,float grow,
            int u,int v,int textureWidth,int textureHeight,boolean mirror) {
        float x0=x-grow,y0=y-grow,z0=z-grow,x1=x+width+grow,y1=y+height+grow,z1=z+depth+grow;
        // Cube swaps its X endpoints before constructing the vertices.
        if(mirror){float swap=x0;x0=x1;x1=swap;}
        points=new float[][]{
            {x0,y0,z0},{x1,y0,z0},{x1,y1,z0},{x0,y1,z0},
            {x0,y0,z1},{x1,y0,z1},{x1,y1,z1},{x0,y1,z1}
        };
        float u1=u+depth,u2=u+depth+width,u3=u+depth+width+width;
        float u4=u+depth+width+depth,u5=u+depth+width+depth+width,v1=v+depth,v2=v+depth+height;
        uv=new float[][]{
            {u1,v,u2,v1},{u2,v1,u3,v},{u,v1,u1,v2},
            {u1,v1,u2,v2},{u2,v1,u4,v2},{u4,v1,u5,v2}
        };
        this.textureWidth=textureWidth;this.textureHeight=textureHeight;this.mirror=mirror;
    }

    void emit(Xform target) {
        emit(target,Xform.FULL_BRIGHT);
    }

    void emit(Xform target,int light) {
        for(int face=0;face<FACES.length;face++) {
            float[] normal=NORMALS[face],rect=uv[face];
            float nx=normal[0];if(mirror&&nx!=0)nx=-nx;
            target.normal(nx,normal[1],normal[2]);
            nx=target.x;float ny=target.y,nz=target.z;
            for(int emitted=0;emitted<4;emitted++) {
                // Polygon assigns UVs, then reverses the complete vertices
                // for a mirror. UVs travel with their vertex during reversal.
                int corner=mirror?3-emitted:emitted;
                float[] point=points[FACES[face][corner]];
                target.position(point[0]/16,point[1]/16,point[2]/16);
                float u=rect[corner==0||corner==3?2:0]/textureWidth;
                float v=rect[corner<2?1:3]/textureHeight;
                target.vertex(target.x,target.y,target.z,-1,u,v,light,nx,ny,nz);
            }
        }
    }
}
