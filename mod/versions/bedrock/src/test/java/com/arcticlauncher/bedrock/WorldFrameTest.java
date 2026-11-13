package com.arcticlauncher.bedrock;

import static org.junit.jupiter.api.Assertions.*;
import com.google.gson.*;
import java.util.*;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import java.io.File;

class WorldFrameTest {
    @TempDir File config;
    private static final String ID="b50ad385-829d-3141-a216-7e7d7539ba7f";
    private static JsonObject fixture() {
        return new JsonParser().parse("{\"schema\":1,\"width\":2,\"height\":2,\"session\":\"offline-test\",\"sequence\":1,\"clockMs\":0,\"localPlayer\":\""+ID+"\",\"server\":\"localhost\",\"dimension\":\"overworld\",\"viewProjection\":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],\"camera\":[0,0,0,0,0,70],\"players\":[{\"id\":\""+ID+"\",\"name\":\"Notch\",\"position\":[1,2,3,0,0],\"model\":[1,0,0,0,0,1,0,0,0,0,1,0],\"skin\":\"engine:test\",\"visible\":true,\"replaceBody\":true}]}").getAsJsonObject();
    }
    @Test void worldIdentityComesFromSnapshotAndClearsOnDisconnect() {
        BedrockPlatform platform=new BedrockPlatform(config,"test","ArcticTest");
        assertFalse(platform.inWorld());assertNull(platform.worldPlayerId());
        UUID profile=platform.playerId();
        platform.world=WorldFrame.parse(fixture().toString(),new float[]{1,1,1,1});
        platform.world.clockMs=1250;
        assertEquals(1250,platform.animationTimeMillis());
        assertTrue(platform.inWorld());assertEquals(UUID.fromString(ID),platform.worldPlayerId());
        assertEquals(profile,platform.playerId());assertArrayEquals(new double[]{1,2,3,0,0},platform.position());
        double[] copy=platform.position();copy[0]=99;assertEquals(1,platform.position()[0]);
        platform.world=null;assertFalse(platform.inWorld());assertNull(platform.worldPlayerId());assertNull(platform.position());
    }
    @Test void rejectsMissingDepthDuplicatePlayersAndInvalidTransforms() {
        JsonObject json=fixture();
        assertThrows(IllegalArgumentException.class,()->WorldFrame.parse(json.toString(),null));
        json.getAsJsonArray("players").add(json.getAsJsonArray("players").get(0));
        assertThrows(IllegalArgumentException.class,()->WorldFrame.parse(json.toString(),new float[]{1,1,1,1}));
        JsonObject invalid=fixture();invalid.add("viewProjection",new JsonArray());
        assertThrows(IllegalArgumentException.class,()->WorldFrame.parse(invalid.toString(),new float[]{1,1,1,1}));
        assertThrows(IllegalArgumentException.class,()->WorldFrame.parse(fixture().toString(),new float[]{Float.NaN,1,1,1}));
    }
    @Test void diagnosticDepthIsAnOwnedCopyOfTheMatchedInput() {
        float[] source={0.25f,0.5f,0.75f,1};
        WorldFrame frame=WorldFrame.parse(fixture().toString(),source);
        source[0]=1;
        float[] copy=frame.depthSnapshot();assertArrayEquals(new float[]{0.25f,0.5f,0.75f,1},copy);
        copy[1]=1;assertEquals(0.5f,frame.depthSnapshot()[1]);
    }
    @Test void attachmentMasksRequireCompletePosesBeforeReplacingNativeRenderers() {
        JsonObject json=fixture(),p=json.getAsJsonArray("players").get(0).getAsJsonObject();
        p.addProperty("availableParts",3);
        assertThrows(IllegalArgumentException.class,()->WorldFrame.parse(json.toString(),new float[]{1,1,1,1}));
        p.addProperty("replaceBody",false);
        assertEquals(3,WorldFrame.parse(json.toString(),new float[]{1,1,1,1}).players[0].availableParts);
        p.addProperty("availableParts",1);p.addProperty("replaceCape",true);
        assertThrows(IllegalArgumentException.class,()->WorldFrame.parse(json.toString(),new float[]{1,1,1,1}));
    }
    @Test void nativePartMatricesRejectScaleReflectionAndMalformedShapes() {
        JsonObject json=fixture(),p=json.getAsJsonArray("players").get(0).getAsJsonObject();
        float[][] matrices=new float[6][];
        for(int i=0;i<6;i++) matrices[i]=AvatarRenderer.identity();
        p.add("partMatrices",new Gson().toJsonTree(matrices));
        assertNotNull(WorldFrame.parse(json.toString(),new float[]{1,1,1,1}).players[0].partMatrices);
        matrices[0][0]=2;p.add("partMatrices",new Gson().toJsonTree(matrices));
        assertThrows(IllegalArgumentException.class,()->WorldFrame.parse(json.toString(),new float[]{1,1,1,1}));
        matrices[0][0]=-1;p.add("partMatrices",new Gson().toJsonTree(matrices));
        assertThrows(IllegalArgumentException.class,()->WorldFrame.parse(json.toString(),new float[]{1,1,1,1}));
        p.add("partMatrices",new Gson().toJsonTree(Arrays.copyOf(matrices,5)));
        assertThrows(IllegalArgumentException.class,()->WorldFrame.parse(json.toString(),new float[]{1,1,1,1}));
    }
}
