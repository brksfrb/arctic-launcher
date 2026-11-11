package com.arcticlauncher.bedrock;

import static org.junit.jupiter.api.Assertions.*;
import java.nio.file.Path;
import java.io.IOException;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class OfflineProfileTest {
    @TempDir Path temporary;

    @Test void usesTheSameOfflineUuidAsJavaWithoutInventingAnInWorldPlayer() {
        BedrockPlatform p = new BedrockPlatform(temporary.toFile(), "test", "Notch");
        assertEquals("b50ad385-829d-3141-a216-7e7d7539ba7f", p.playerId().toString());
        assertEquals("Notch", p.playerName());
        assertFalse(p.inWorld());
        assertNull(p.worldPlayerId());
        assertNull(p.position());
        assertThrows(IOException.class, () -> p.joinServer("must-not-authenticate"));
    }

    @Test void refusesAnInvalidOfflineIdentityBeforeInitializingTheClient() {
        assertThrows(IllegalArgumentException.class,
                () -> new BedrockPlatform(temporary.toFile(), "test", "../account"));
        assertThrows(IllegalArgumentException.class,
                () -> new BedrockPlatform(temporary.toFile(), "test", null));
    }
}
