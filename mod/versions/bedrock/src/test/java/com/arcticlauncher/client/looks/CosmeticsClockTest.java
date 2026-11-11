package com.arcticlauncher.client.looks;

import com.arcticlauncher.client.Platform;
import com.google.gson.JsonParser;
import java.lang.reflect.Proxy;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicLong;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class CosmeticsClockTest {
    @Test void emotesFollowAdapterTimeAndWorldResetClearsPlayback() {
        AtomicLong clock = new AtomicLong(1000);
        Platform platform = (Platform)Proxy.newProxyInstance(Platform.class.getClassLoader(),
                new Class<?>[]{Platform.class}, (proxy, method, arguments) -> {
                    if(method.getName().equals("animationTimeMillis")) return clock.get();
                    throw new AssertionError("unexpected platform call: "+method.getName());
                });
        Cosmetics cosmetics = new Cosmetics(platform,"http://127.0.0.1:9",null);
        Cosmetics.Emote emote = new Cosmetics.Emote("wave","Wave",null);
        emote.animation = Animation.parse(new JsonParser().parse(
                "{\"animations\":{\"wave\":{\"animation_length\":1,\"loop\":false,"+
                "\"bones\":{\"rightArm\":{\"rotation\":{\"0\":[0,0,0],\"1\":[0,0,90]}}}}}}"));
        UUID id=UUID.nameUUIDFromBytes("OfflinePlayer:ArcticTest".getBytes(java.nio.charset.StandardCharsets.UTF_8));
        cosmetics.playLocal(id,emote);
        Cosmetics.Playing playing=cosmetics.playingFor(id);
        assertNotNull(playing);
        assertEquals(0,playing.seconds());
        clock.set(1250);assertEquals(.25f,playing.seconds());
        float[] original=emote.animation.rotation("rightArm",playing.seconds());
        clock.set(1750);assertEquals(.75f,playing.seconds());
        assertFalse(java.util.Arrays.equals(original,emote.animation.rotation("rightArm",playing.seconds())));
        clock.set(1250);assertArrayEquals(original,emote.animation.rotation("rightArm",playing.seconds()));
        clock.set(999);assertEquals(0,playing.seconds());
        clock.set(2000);assertNull(cosmetics.playingFor(id));
        cosmetics.playLocal(id,emote);
        cosmetics.clearPlayback();assertNull(cosmetics.playingFor(id));
        cosmetics.holdLocal(id,emote,.4f);clock.set(9000);
        assertEquals(.4f,cosmetics.playingFor(id).seconds());
    }
}
