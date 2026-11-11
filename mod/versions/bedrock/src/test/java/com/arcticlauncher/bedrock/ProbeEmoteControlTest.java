package com.arcticlauncher.bedrock;

import com.google.gson.*;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import static org.junit.jupiter.api.Assertions.*;

class ProbeEmoteControlTest {
    @TempDir Path root;
    @Test void controlsRequireEnabledProbeAndRejectUnknownActionsBeforePlayback()throws Exception {
        Path input=root.resolve("probe-emote-control.json"),output=root.resolve("probe-emote-control-result.json");
        Files.write(input,"{\"schema\":1,\"sequence\":1,\"action\":\"arbitrary\"}".getBytes(StandardCharsets.UTF_8));
        new ProbeEmoteControl(root,false).poll(null);assertFalse(Files.exists(output));
        ProbeEmoteControl control=new ProbeEmoteControl(root,true);control.poll(null);
        JsonObject ack=new JsonParser().parse(new String(Files.readAllBytes(output),StandardCharsets.UTF_8)).getAsJsonObject();
        assertEquals(1,ack.get("sequence").getAsLong());assertTrue(ack.has("error"));
        Files.delete(output);
        Files.write(input,"{\"schema\":1,\"sequence\":2,\"action\":\"play\"}".getBytes(StandardCharsets.UTF_8));
        control.poll(null);assertFalse(Files.exists(output)); // No live player cannot acknowledge playback.
    }
}
