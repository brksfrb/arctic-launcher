package com.arcticlauncher.bedrock;

import java.awt.image.BufferedImage;
import java.io.IOException;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

/** Same content domain as native_skins::image: little-endian width/height,
 * then row-major RGBA bytes. PNG encoding and native addresses are irrelevant. */
final class NativeImageIdentity {
    private NativeImageIdentity() {}
    static String capeKey(BufferedImage image) throws IOException {
        if(image==null||image.getWidth()<64||image.getWidth()>1024||image.getWidth()%64!=0
                ||image.getHeight()*2!=image.getWidth())throw new IOException("invalid standard native cape image");
        try {
            MessageDigest hash=MessageDigest.getInstance("SHA-256");
            int width=image.getWidth(),height=image.getHeight();
            for(int size:new int[]{width,height})for(int shift=0;shift<32;shift+=8)hash.update((byte)(size>>>shift));
            int[] pixels=new int[width];byte[] row=new byte[width*4];
            for(int y=0;y<height;y++) {
                image.getRGB(0,y,width,1,pixels,0,width);
                for(int x=0;x<width;x++) {
                    int pixel=pixels[x],at=x*4;
                    row[at]=(byte)(pixel>>>16);row[at+1]=(byte)(pixel>>>8);row[at+2]=(byte)pixel;row[at+3]=(byte)(pixel>>>24);
                }
                hash.update(row);
            }
            char[] digits="0123456789abcdef".toCharArray();byte[] digest=hash.digest();char[] text=new char[digest.length*2];
            for(int i=0;i<digest.length;i++){text[i*2]=digits[(digest[i]>>>4)&15];text[i*2+1]=digits[digest[i]&15];}
            return "engine:cape/"+new String(text);
        }catch(NoSuchAlgorithmException missing){throw new IOException(missing);}
    }
}
