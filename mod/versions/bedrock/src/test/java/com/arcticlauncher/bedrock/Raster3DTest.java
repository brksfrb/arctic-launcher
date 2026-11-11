package com.arcticlauncher.bedrock;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.looks.Xform;
import java.awt.image.BufferedImage;
import java.util.Arrays;
import org.junit.jupiter.api.Test;

class Raster3DTest {
    private static final float[] CAMERA={1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1};
    private static void quad(Raster3D raster,float z,int argb,BufferedImage texture,boolean blend) {
        Xform sink=raster.sink(AvatarRenderer.identity(),texture,blend,true);
        vertex(sink,-1,-1,z,argb,0,1);vertex(sink,1,-1,z,argb,1,1);
        vertex(sink,1,1,z,argb,1,0);vertex(sink,-1,1,z,argb,0,0);
    }
    private static void vertex(Xform x,float px,float py,float pz,int argb,float u,float v) {
        x.vertex(px,py,pz,argb,u,v,0,0,0,-1);
    }
    @Test void opaqueSurfacesUseDepthRegardlessOfSubmissionOrder() {
        for(boolean nearFirst:new boolean[]{false,true}) {
            Raster3D r=new Raster3D(16,16,CAMERA,null);
            if(nearFirst) quad(r,.2f,0xFFFF0000,null,false);
            quad(r,.8f,0xFF0000FF,null,false);
            if(!nearFirst) quad(r,.2f,0xFFFF0000,null,false);
            assertEquals(0xFFFF0000,r.finish().getRGB(8,8));
        }
    }
    @Test void terrainDepthAndTransparentTexelsDoNotRevealOccludedGeometry() {
        float[] terrain=new float[256];Arrays.fill(terrain,1f);
        for(int y=0;y<16;y++) for(int x=0;x<8;x++) terrain[y*16+x]=.1f;
        Raster3D r=new Raster3D(16,16,CAMERA,terrain);
        BufferedImage cutout=new BufferedImage(2,1,BufferedImage.TYPE_INT_ARGB);
        cutout.setRGB(0,0,0);cutout.setRGB(1,0,0xFFFF0000);
        quad(r,.2f,0xFFFFFFFF,cutout,false);
        quad(r,.8f,0xFF0000FF,null,false);
        BufferedImage result=r.finish();
        assertEquals(0,result.getRGB(4,8));
        assertEquals(0xFFFF0000,result.getRGB(12,8));
        Raster3D holes=new Raster3D(16,16,CAMERA,null);
        quad(holes,.2f,0xFFFFFFFF,cutout,false);
        quad(holes,.8f,0xFF0000FF,null,false);
        assertEquals(0xFF0000FF,holes.finish().getRGB(4,8));
    }
    @Test void translucentQuadsHaveNoDoubleBlendedDiagonalAndSortBehindNearerOpaqueFaces() {
        Raster3D r=new Raster3D(16,16,CAMERA,null);
        quad(r,.3f,0x80FF0000,null,true);
        quad(r,.7f,0xFF0000FF,null,false);
        BufferedImage image=r.finish();
        for(int y=0;y<16;y++) for(int x=0;x<16;x++) assertEquals(0xFF80007F,image.getRGB(x,y));
        Raster3D hidden=new Raster3D(16,16,CAMERA,null);
        quad(hidden,.8f,0x80FF0000,null,true);
        quad(hidden,.1f,0xFF0000FF,null,false);
        assertEquals(0xFF0000FF,hidden.finish().getRGB(8,8));
    }
    @Test void clipsNearPlaneAndDiscardsAnEntireQuadContainingNonFiniteVertices() {
        Raster3D r=new Raster3D(16,16,CAMERA,null);
        Xform sink=r.sink(AvatarRenderer.identity(),null,false,true);
        vertex(sink,-1,-1,-.5f,-1,0,0);vertex(sink,1,-1,.5f,-1,1,0);
        vertex(sink,1,1,.5f,-1,1,1);vertex(sink,-1,1,-.5f,-1,0,1);
        BufferedImage image=r.finish();
        assertEquals(0,image.getRGB(2,8));assertEquals(-1,image.getRGB(13,8));
        Raster3D bad=new Raster3D(16,16,CAMERA,null);
        sink=bad.sink(AvatarRenderer.identity(),null,false,true);
        vertex(sink,Float.NaN,-1,.5f,-1,0,0);vertex(sink,1,-1,.5f,-1,1,0);
        vertex(sink,1,1,.5f,-1,1,1);vertex(sink,-1,1,.5f,-1,0,1);
        vertex(sink,-1,-1,.7f,0xFF123456,0,0);vertex(sink,1,-1,.7f,0xFF123456,1,0);
        vertex(sink,1,1,.7f,0xFF123456,1,1);vertex(sink,-1,1,.7f,0xFF123456,0,1);
        BufferedImage corrected=bad.finish();
        for(int y=0;y<16;y++) for(int x=0;x<16;x++) assertEquals(0xFF123456,corrected.getRGB(x,y));
    }
    @Test void perspectiveCorrectTextureCoordinatesDifferFromScreenLinearInterpolation() {
        // w=z+1; the right edge is twice as far away. At screen x=0,
        // perspective-correct u is 1/2; linear screen interpolation would be 2/3.
        float[] camera=CAMERA.clone();camera[11]=1;
        Raster3D r=new Raster3D(100,100,camera,null);
        BufferedImage texture=new BufferedImage(10,1,BufferedImage.TYPE_INT_ARGB);
        for(int i=0;i<10;i++) texture.setRGB(i,0,0xFF000000|i);
        Xform sink=r.sink(AvatarRenderer.identity(),texture,false,true);
        vertex(sink,-1,-1,0,-1,0,0);vertex(sink,1,-1,1,-1,1,0);
        vertex(sink,1,1,1,-1,1,0);vertex(sink,-1,1,0,-1,0,0);
        assertEquals(5,r.finish().getRGB(50,50)&255);
    }
}
