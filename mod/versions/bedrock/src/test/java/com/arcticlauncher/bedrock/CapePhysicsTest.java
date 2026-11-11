package com.arcticlauncher.bedrock;

import static org.junit.jupiter.api.Assertions.*;
import java.util.Arrays;
import java.awt.image.BufferedImage;
import org.junit.jupiter.api.Test;

class CapePhysicsTest {
    private static CapePhysics.Motion motion(double x,double y,double z) {
        CapePhysics.Motion m=new CapePhysics.Motion();m.delta=new double[]{x,y,z};return m;
    }
    @Test void restForwardSideAndVerticalMotionUseJavaLimits() {
        assertArrayEquals(new float[]{6,0,180},CapePhysics.angles(motion(0,0,0)),.0001f);
        assertArrayEquals(new float[]{81,0,180},CapePhysics.angles(motion(0,0,-10)),.0001f);
        assertArrayEquals(new float[]{6,-10,190},CapePhysics.angles(motion(10,0,0)),.0001f);
        assertArrayEquals(new float[]{38,0,180},CapePhysics.angles(motion(0,10,0)),.0001f);
        assertArrayEquals(new float[]{0,0,180},CapePhysics.angles(motion(0,-10,0)),.0001f);
        CapePhysics.Motion m=motion(0,0,-10);m.fallFlyingScale=1;
        assertEquals(6,CapePhysics.angles(m)[0],.0001f);
        m=motion(0,0,0);m.walkDistance=(float)Math.PI/12;m.bob=.5f;
        assertEquals(22,CapePhysics.angles(m)[0],.0001f);
    }
    @Test void chasingPositionIsTickBasedInterpolatedAndResetsAfterTeleport() {
        CapePhysics.Chaser c=new CapePhysics.Chaser();c.reset(0,0,0);c.tick(0,0,1);
        assertArrayEquals(new double[]{0,0,-.75},c.delta(1),1e-9);
        assertArrayEquals(new double[]{0,0,-.375},c.delta(.5f),1e-9);
        double[] first=c.delta(.5f);assertArrayEquals(first,c.delta(.5f));
        c.tick(0,0,1);assertEquals(-.5625,c.delta(1)[2],1e-9);
        c.tick(100,0,1);assertEquals(0,c.delta(1)[0],1e-9);
        c.reset(0,0,0);c.tick(0,0,1);assertArrayEquals(first,c.delta(.5f));
        assertThrows(IllegalArgumentException.class,()->c.tick(Double.NaN,0,0));
    }
    @Test void physicsChangesRenderedCapeAndElytraSuppressesIt() {
        AvatarRenderer.Avatar avatar=new AvatarRenderer.Avatar();
        avatar.cape=new BufferedImage(64,32,BufferedImage.TYPE_INT_ARGB);
        for(int y=0;y<32;y++)for(int x=0;x<64;x++)avatar.cape.setRGB(x,y,0xFFFF0055);
        int[] resting=render(avatar);assertTrue(Arrays.stream(resting).anyMatch(v->v!=0));
        avatar.capeMotion=motion(.1,.4,-.8);assertFalse(Arrays.equals(resting,render(avatar)));
        int[] posed=render(avatar);assertArrayEquals(posed,render(avatar));
        avatar.elytra=true;assertTrue(Arrays.stream(render(avatar)).allMatch(v->v==0));
    }
    private static int[] render(AvatarRenderer.Avatar a) {
        return AvatarRenderer.preview(a,180,240,110,.4f,.1f).getRGB(0,0,180,240,null,0,180);
    }
}
