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

	private static List<String> hide(List<String> lines, String... labels) {
		ClientConfig config = new ClientConfig();
		config.debugStyle = true;
		for (String label : labels) {
			for (int i = 0; i < DebugStyle.GROUPS.length; i++) {
				if (DebugStyle.GROUPS[i].label.equals(label)) {
					DebugStyle.setHidden(config, i, true);
				}
			}
		}
		return DebugStyle.without(lines, config.debugHidden);
	}

	@Test
	void hiddenGroupsAreLeftOutAndTheSpacingClosesUp() {
		List<String> lines = Arrays.asList("Minecraft 1.21.4 (vanilla)", "144 fps T: inf", "", "XYZ: 1.0 / 64.0 / 2.0", "Block: 1 64 2", "",
				"Facing: north", "Biome: minecraft:plains");
		assertEquals(Arrays.asList("Minecraft 1.21.4 (vanilla)", "", "Facing: north", "Biome: minecraft:plains"),
				hide(lines, "FPS", "Coordinates"));
		assertEquals(Arrays.asList("144 fps T: inf", "", "XYZ: 1.0 / 64.0 / 2.0", "Block: 1 64 2"),
				hide(lines, "Version", "Facing", "Biome and difficulty"));
	}

	@Test
	void aLineTheGroupsDoNotKnowFollowsTheLineBeforeIt() {
		List<String> lines = Arrays.asList("Java: 21.0.7", "", "Targeted Block: 1, 64, 2", "minecraft:grass_block", "facing: north", "",
				"Debug charts: [F3+1] Profiler hidden");
		assertEquals(Arrays.asList("Java: 21.0.7", "", "Debug charts: [F3+1] Profiler hidden"), hide(lines, "Targeted block"));
	}

	@Test
	void coloredLinesAreMatchedByTheirPlainText() {
		assertEquals(Arrays.asList("keep"), hide(Arrays.asList(S + "bXYZ: 1 2 3", "", "keep"), "Coordinates"));
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
