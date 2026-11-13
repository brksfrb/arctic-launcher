package com.arcticlauncher.bedrock;

import static org.junit.jupiter.api.Assertions.*;
import java.awt.image.BufferedImage;
import org.junit.jupiter.api.Test;

class NativeTextureTest {
    @Test void nativeLegacySkinUsesSharedMirroringOpacityAndOwnedPixelConversion() {
        byte[] rgba=new byte[64*32*4];
        for(int i=0;i<rgba.length;i+=4){rgba[i]=12;rgba[i+1]=34;rgba[i+2]=56;rgba[i+3]=(byte)255;}
        BufferedImage skin=NativeBridge.engineImage("engine:skin/offline",64,32,rgba);
        assertEquals(64,skin.getHeight());
        assertEquals(0xFF0C2238,skin.getRGB(4,20));
        assertEquals(skin.getRGB(4,20),skin.getRGB(20,52));
        assertEquals(0,skin.getRGB(40,4)>>>24);
        assertEquals((byte)255,rgba[3]);
        rgba[0]=99;assertEquals(12,(skin.getRGB(0,0)>>>16)&255);
        BufferedImage cape=NativeBridge.engineImage("engine:cape/offline",64,32,new byte[64*32*4]);
        assertEquals(32,cape.getHeight());assertEquals(0,cape.getRGB(4,20));
    }
    @Test void engineSkinRejectsIncompatibleLayoutsWithoutChangingGeneralTextureSupport() {
        assertThrows(IllegalArgumentException.class,()->NativeBridge.engineImage("engine:skin/bad",96,96,new byte[96*96*4]));
        assertEquals(96,NativeBridge.engineImage("engine:other",96,96,new byte[96*96*4]).getWidth());
        assertThrows(IllegalArgumentException.class,()->NativeBridge.engineImage("engine:skin/bad",64,64,new byte[1]));
    }
}
