package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;

import com.arcticlauncher.client.config.ClientConfig;
import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.Test;

class DebugStyleTest {
	private static final char S = '§';

	@Test
	void aLabelAndItsValueGetTheirOwnColors() {
		assertEquals("" + S + "bXYZ" + S + "7: " + S + "f1.0 / 64.0 / 2.0", DebugStyle.line("XYZ: 1.0 / 64.0 / 2.0", 'b', 'f'));
	}

	@Test
	void linesWithoutALabelOrAlreadyColoredStayAsTheyAre() {
		assertEquals("Minecraft 1.21 (vanilla)", DebugStyle.line("Minecraft 1.21 (vanilla)", 'b', 'f'));
		assertEquals(S + "cRed: text", DebugStyle.line(S + "cRed: text", 'b', 'f'));
		assertEquals("", DebugStyle.line("", 'b', 'f'));
		String sentence = "This is a long sentence before a colon that is not a label: yes";
		assertEquals(sentence, DebugStyle.line(sentence, 'b', 'f'));
	}

	@Test
	void nothingChangesWhileTheModIsOff() {
		ClientConfig config = new ClientConfig();
		List<String> lines = Arrays.asList("FPS: 60");
		assertSame(lines, DebugStyle.apply(lines, config));
		config.debugStyle = true;
		assertEquals(Arrays.asList(S + "bFPS" + S + "7: " + S + "f60"), DebugStyle.apply(lines, config));
	}
}
