package com.arcticlauncher.bedrock;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.looks.*;
import com.google.gson.JsonParser;
import java.awt.image.BufferedImage;
import java.util.Arrays;
import java.nio.*;
import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

class AvatarRendererTest {
    private static BufferedImage color(int w,int h,int argb) {
        BufferedImage image=new BufferedImage(w,h,BufferedImage.TYPE_INT_ARGB);
        for(int y=0;y<h;y++) for(int x=0;x<w;x++) image.setRGB(x,y,argb);
        return image;
    }
    private static int[] pixels(AvatarRenderer.Avatar a) {
        return AvatarRenderer.preview(a,180,240,110,.3f,.1f).getRGB(0,0,180,240,null,0,180);
    }
    private static Animation animation(String bones) {
        return Animation.parse(new JsonParser().parse("{\"animations\":{\"test\":{\"animation_length\":1,\"loop\":true,\"bones\":{"+bones+"}}}}"));
    }
    @Test void classicAndSlimSkinsRenderDifferentArmWidths() {
        AvatarRenderer.Avatar a=new AvatarRenderer.Avatar();a.skin=color(64,64,0xFF40A0F0);
        int[] classic=pixels(a);a.slim=true;
        int[] slim=pixels(a);
        assertTrue(Arrays.stream(classic).filter(p->p!=0).count()>5000);
        assertTrue(Arrays.stream(classic).filter(p->p!=0).count()>Arrays.stream(slim).filter(p->p!=0).count());
    }
    @Test void expressiveRootAndSplitArmEmotesRenderOnARepeatableClock() {
        AvatarRenderer.Avatar a=new AvatarRenderer.Avatar();a.skin=color(64,64,0xFFE0A050);
        int[] resting=pixels(a);
        a.emote=animation("\"torso\":{\"rotation\":[30,0,0]},\"rightForearm\":{\"rotation\":{\"0\":[0,0,0],\"1\":[0,0,100]}}");
        a.emoteSeconds=.1f;int[] first=pixels(a);
        assertFalse(Arrays.equals(resting,first));
        a.emoteSeconds=.7f;int[] later=pixels(a);
        assertFalse(Arrays.equals(first,later));
        a.emoteSeconds=.1f;assertArrayEquals(first,pixels(a));
    }
    @Test void attachedCuboidsUseIdleAnimationFitAndMirroring() {
        Geometry geometry=Geometry.parse(new JsonParser().parse("{\"minecraft:geometry\":[{\"description\":{\"texture_width\":16,\"texture_height\":16},\"bones\":[{\"name\":\"head\",\"pivot\":[0,24,0],\"cubes\":[{\"origin\":[3,28,-7],\"size\":[4,4,4],\"uv\":[0,0]}]}]}]}"));
        AvatarRenderer.Cosmetic cosmetic=AvatarRenderer.cuboid(geometry,animation("\"head\":{\"position\":{\"0\":[0,0,0],\"1\":[3,0,0]}}"),color(16,16,0xFFFF00FF),null);
        AvatarRenderer.Avatar a=new AvatarRenderer.Avatar();
        a.worn.add(new AvatarRenderer.Worn(cosmetic,Fit.NONE));a.seconds=.1f;
        int[] normal=pixels(a);assertTrue(Arrays.stream(normal).anyMatch(p->p!=0));
        a.seconds=.7f;assertFalse(Arrays.equals(normal,pixels(a)));
        a.seconds=.1f;a.worn.clear();a.worn.add(new AvatarRenderer.Worn(cosmetic,Fit.of(4,2,true)));
        assertFalse(Arrays.equals(normal,pixels(a)));
        a.reduceMotion=true;int[] reduced=pixels(a);a.seconds=.9f;assertFalse(Arrays.equals(reduced,pixels(a)));
    }
    @Test void animatedCapeTexturesSelectFramesByExplicitClockAndRespectFreeze() {
        ClientConfig config=new ClientConfig();
        // Constructing Looks does not start its background worker or any account/network request.
        Looks looks=new Looks(null,config,"http://127.0.0.1:9",null);
        looks.textureReady("cape",2);
        assertEquals("cape/0",looks.frameAt("cape",0));
        assertEquals("cape/1",looks.frameAt("cape",125));
        assertEquals("cape/0",looks.frameAt("cape",250));
        AvatarRenderer.Avatar a=new AvatarRenderer.Avatar();a.cape=color(64,32,0xFFFF0000);
        int[] red=pixels(a);a.cape=color(64,32,0xFF0000FF);
        assertFalse(Arrays.equals(red,pixels(a)));
        config.reduceCapeMotion=true;assertEquals("cape/0",looks.frameAt("cape",125));
    }
    @Test void unavailableAttachmentsCannotDrawGuessedCosmeticPoses() {
        Geometry geometry=Geometry.parse(new JsonParser().parse("{\"minecraft:geometry\":[{\"description\":{\"texture_width\":16,\"texture_height\":16},\"bones\":[{\"name\":\"head\",\"pivot\":[0,24,0],\"cubes\":[{\"origin\":[-3,25,-5],\"size\":[6,6,2],\"uv\":[0,0]}]}]}]}"));
        AvatarRenderer.Avatar a=new AvatarRenderer.Avatar();
        a.worn.add(new AvatarRenderer.Worn(AvatarRenderer.cuboid(geometry,null,color(16,16,0xFF00FF00),null),Fit.NONE));
        assertTrue(Arrays.stream(pixels(a)).anyMatch(p->p!=0));
        a.availableParts=2;assertTrue(Arrays.stream(pixels(a)).allMatch(p->p==0));
        a.availableParts=1;assertTrue(Arrays.stream(pixels(a)).anyMatch(p->p!=0));
    }
    @Test void nativeMatricesReachAttachmentsAndUnposedEmoteLimbsKeepTheirNativeBaseline() {
        AvatarRenderer.Avatar a=new AvatarRenderer.Avatar();a.partMatrices=new float[6][];
        for(Attach part:Attach.values()) {
            float[] m=AvatarRenderer.identity();m[3]=part.x/16;m[7]=part.y/16;m[11]=part.z/16;
            a.partMatrices[part.ordinal()]=m;
        }
        float[] arm=a.partMatrices[Attach.RIGHT_ARM.ordinal()];
        arm[0]=0;arm[1]=-1;arm[4]=1;arm[5]=0;
        final float[][][] observed=new float[1][][];
        a.worn.add(new AvatarRenderer.Worn((raster,parts,fit,time,animate,light)->observed[0]=parts,Fit.NONE));
        Raster3D raster=new Raster3D(2,2,new float[]{1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1},null);
        AvatarRenderer.emit(a,raster,AvatarRenderer.identity());
        assertArrayEquals(arm,observed[0][Attach.RIGHT_ARM.ordinal()]);
        a.emote=animation("\"head\":{\"rotation\":[0,30,0]}");
        AvatarRenderer.emit(a,raster,AvatarRenderer.identity());
        assertArrayEquals(arm,observed[0][Attach.RIGHT_ARM.ordinal()],.00001f);
        assertFalse(Arrays.equals(a.partMatrices[0],observed[0][0]));
        a.availableParts=1;
        AvatarRenderer.emit(a,raster,AvatarRenderer.identity());
        assertNull(observed[0][Attach.RIGHT_ARM.ordinal()]);
    }
    @Test void vertexColoredGlbMeshesAnimateAndFreezeOnTheSuppliedClock() throws Exception {
        MeshModel mesh=MeshModel.parse(meshFixture());
        AvatarRenderer.Avatar a=new AvatarRenderer.Avatar();
        a.worn.add(new AvatarRenderer.Worn(AvatarRenderer.mesh(mesh),Fit.NONE));
        a.seconds=.1f;int[] first=pixels(a);
        assertTrue(Arrays.stream(first).filter(p->p!=0).count()>500);
        assertTrue(Arrays.stream(first).anyMatch(p->((p>>>16)&255)>100&&(p&255)<50));
        a.seconds=.7f;assertFalse(Arrays.equals(first,pixels(a)));
        a.seconds=.1f;assertArrayEquals(first,pixels(a));
        a.reduceMotion=true;int[] rest=pixels(a);a.seconds=.9f;assertArrayEquals(rest,pixels(a));
        assertArrayEquals(MeshDraw.poseAt(mesh,false,.1f)[0],MeshDraw.poseAt(mesh,false,.9f)[0]);
    }
    private static byte[] meshFixture() {
        // A real GLB file: a colored triangle and a translation channel. No
        // downloaded assets or constructor shortcuts bypass the shared reader.
        String json="{\"asset\":{\"version\":\"2.0\"},\"scene\":0,\"scenes\":[{\"nodes\":[0]}],"
            +"\"nodes\":[{\"name\":\"head\",\"mesh\":0}],\"meshes\":[{\"primitives\":[{\"attributes\":{\"POSITION\":0,\"COLOR_0\":1}}]}],"
            +"\"buffers\":[{\"byteLength\":116}],\"bufferViews\":[{\"buffer\":0,\"byteOffset\":0,\"byteLength\":36},{\"buffer\":0,\"byteOffset\":36,\"byteLength\":48},{\"buffer\":0,\"byteOffset\":84,\"byteLength\":8},{\"buffer\":0,\"byteOffset\":92,\"byteLength\":24}],"
            +"\"accessors\":[{\"bufferView\":0,\"componentType\":5126,\"count\":3,\"type\":\"VEC3\"},{\"bufferView\":1,\"componentType\":5126,\"count\":3,\"type\":\"VEC4\"},{\"bufferView\":2,\"componentType\":5126,\"count\":2,\"type\":\"SCALAR\"},{\"bufferView\":3,\"componentType\":5126,\"count\":2,\"type\":\"VEC3\"}],"
            +"\"animations\":[{\"samplers\":[{\"input\":2,\"output\":3}],\"channels\":[{\"sampler\":0,\"target\":{\"node\":0,\"path\":\"translation\"}}]}]}";
        while(json.length()%4!=0)json+=" ";
        byte[] utf8=json.getBytes(StandardCharsets.UTF_8);
        ByteBuffer glb=ByteBuffer.allocate(28+utf8.length+116).order(ByteOrder.LITTLE_ENDIAN);
        glb.putInt(0x46546C67).putInt(2).putInt(glb.capacity());
        glb.putInt(utf8.length).putInt(0x4E4F534A).put(utf8).putInt(116).putInt(0x004E4942);
        for(float f:new float[]{-4,24,-4,4,24,-4,0,32,-4, 1,0,0,1,0,1,0,1,0,0,1,1, 0,1, 0,0,0,5,0,0})glb.putFloat(f);
        return glb.array();
    }
}
