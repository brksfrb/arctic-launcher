package com.arcticlauncher.bedrock;

import com.google.gson.*;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import static org.junit.jupiter.api.Assertions.*;

class ProbeBodyControlTest {
    @TempDir Path root;
    private void request(String value)throws Exception {
        Files.write(root.resolve("probe-body-control.json"),value.getBytes(StandardCharsets.UTF_8));
    }
    @Test void modesRequireEnabledProbeAndNewSequence()throws Exception {
        request("{\"schema\":1,\"sequence\":1,\"mode\":\"suppression\",\"alpha\":0.001}");
        assertTrue(new ProbeBodyControl(root,false).poll().renderLayer());
        ProbeBodyControl control=new ProbeBodyControl(root,true);
        ProbeBodyControl.State state=control.poll();assertFalse(state.renderLayer());assertTrue(state.allowReadiness());
        assertEquals(0.001f,state.alpha);
        request("{\"schema\":1,\"sequence\":1,\"mode\":\"native\"}");
        assertFalse(control.poll().renderLayer());
        request("{\"schema\":1,\"sequence\":2,\"mode\":\"native\"}");
        state=control.poll();assertTrue(state.renderLayer());assertFalse(state.allowReadiness());
        JsonObject ack=new JsonParser().parse(new String(Files.readAllBytes(root.resolve("probe-body-control-result.json")),StandardCharsets.UTF_8)).getAsJsonObject();
        assertEquals(2,ack.get("sequence").getAsLong());assertEquals("native",ack.get("mode").getAsString());
    }
    @Test void rejectedControlsRestoreNativeBodyAndPermitLaterRecovery()throws Exception {
        ProbeBodyControl control=new ProbeBodyControl(root,true);
        request("{\"schema\":1,\"sequence\":1,\"mode\":\"suppression\",\"alpha\":0.5}");
        assertFalse(control.poll().allowReadiness());
        request("{\"schema\":1,\"sequence\":2,\"mode\":\"replacement\",\"alpha\":0}");
        assertTrue(control.poll().allowReadiness());assertTrue(control.poll().renderLayer());
        request("{broken");assertFalse(control.poll().allowReadiness());
        request("{\"schema\":1,\"sequence\":3,\"mode\":\"replacement\"}");
        assertTrue(control.poll().allowReadiness());
    }
}
