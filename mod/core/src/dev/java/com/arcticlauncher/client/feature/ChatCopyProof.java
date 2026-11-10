package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.io.File;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.util.LinkedHashMap;
import java.util.Map;

/** Opt-in real-game clipboard regression test, shared by the Fabric and legacy adapters. */
public final class ChatCopyProof {
	public static final String SHORT = "Plain chat: café §acolored§r and literal (x12)";
	public static final String SHORT_PLAIN = "Plain chat: café colored and literal (x12)";
	public static final String URL = "URL https://example.com/test?q=chat";
	public static final String WRAP = "Wrapped start: one two three four five six seven eight nine ten. "
			+ "Eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty. "
			+ "A long message must copy every word across every visual line, including this final ending.";
	public static final String RUN = "Run /help";
	public static final String SUGGEST = "Suggest /help";
	public static final String REPEAT = "Repeated message";
	public static final String[] EXPECTED = {SHORT_PLAIN, URL, WRAP, REPEAT, RUN, SUGGEST};

	public interface Game {
		String hoverText(Object style);
		void hoverAt(int x, int y);
		int width();
		int height();
		Object hit(int x, int y);
		boolean click(int button, int x, int y, boolean shift);
		String action(Object style);
		boolean colored(int x, int y, Object style);
	}

	private ChatCopyProof() {}

	public static void check(Game game, PrintWriter log, String phase) {
		Map<String, int[]> targets = new LinkedHashMap<>();
		Map<String, int[]> bodies = new LinkedHashMap<>();
		int[] colored = null;
		Map<Integer, int[]> wrapped = new LinkedHashMap<>();
		for (int y = Math.max(0, game.height() - 210); y < game.height() - 25; y += 3) {
			for (int x = 3; x < Math.min(320, game.width()); x += 7) {
				Object style = game.hit(x, y);
				ChatCopy.Data data = ChatCopy.data(style);
				if (data == null) continue;
				if (!data.target && SHORT_PLAIN.equals(data.text) && game.colored(x, y, style)) colored = new int[] {x, y};
				if (data.target) {
					require("Click to copy".equals(game.hoverText(style)), "copy target hover text");
					targets.put(data.text, new int[] {x, y});
				}
				else {
					require(game.hoverText(style).isEmpty(), "plain body has no copy hover");
					bodies.put(data.text, new int[] {x, y});
					if (WRAP.equals(data.text)) wrapped.put(y, new int[] {x, y});
				}
			}
		}
		require(colored != null, "visible green inline formatted text");
		copy(game, colored, SHORT_PLAIN, true);
		log.println("PASS " + phase + ": colored word at " + colored[0] + "," + colored[1] + " copied whole message via Shift+right-click");
		for (String text : EXPECTED) {
			int[] target = targets.get(text);
			int[] body = bodies.get(text);
			require(target != null && body != null, "visible target and body for " + text);
			copy(game, target, text, false);
			copy(game, body, text, true);
			ArcticClient.platform().setClipboard("unchanged");
			require(!game.click(0, body[0], body[1], false), "ordinary message click passes through");
			require("unchanged".equals(ArcticClient.platform().clipboard()), "ordinary click preserves clipboard");
		}
		int[] url = bodies.get(URL);
		require("OPEN_URL".equals(game.action(game.hit(url[0], url[1]))), "URL click event preserved");
		for (String text : new String[] {RUN, SUGGEST}) {
			int[] point = bodies.get(text);
			require((text.equals(RUN) ? "RUN_COMMAND" : "SUGGEST_COMMAND").equals(game.action(game.hit(point[0], point[1]))),
					"command click event preserved");
		}
		require(wrapped.size() >= 8, "wrapped message spans multiple rows");
		for (int[] point : wrapped.values()) copy(game, point, WRAP, true);
		ClientConfig config = ArcticClient.config();
		config.chatCopy = false;
		ArcticClient.saveConfig();
		for (String text : EXPECTED) {
			int[] target = targets.get(text), body = bodies.get(text);
			ArcticClient.platform().setClipboard("disabled");
			require(!game.click(0, target[0], target[1], false), "disabled target passes through");
			require(!game.click(1, body[0], body[1], true), "disabled secondary gesture passes through");
			require("disabled".equals(ArcticClient.platform().clipboard()), "disabled clipboard unchanged");
		}
		config.chatCopy = true;
		ArcticClient.saveConfig();
		copy(game, targets.get(WRAP), WRAP, false);
		log.println("PASS " + phase + ": six timestamp/copy targets, green inline text Shift+right-click, " + wrapped.size()
				+ " wrapped samples, formatting removed, literal suffix retained, stacked suffix excluded, URL/run_command/suggest_command preserved, toggle off/on; clipboard read back");
		int[] hover = targets.get(SHORT_PLAIN);
		game.hoverAt(hover[0], hover[1]);
		log.println("PASS target SHOW_TEXT Click to copy; body events preserved; cursor on timestamp/copy target " + hover[0] + "," + hover[1]);
		log.flush();
	}

	private static void copy(Game game, int[] point, String expected, boolean secondary) {
		ArcticClient.platform().setClipboard("before-copy");
		require(game.click(secondary ? 1 : 0, point[0], point[1], secondary), "copy gesture consumed");
		require(expected.equals(ArcticClient.platform().clipboard()), "clipboard equals whole original: " + expected);
	}

	private static void require(boolean condition, String message) {
		if (!condition) throw new AssertionError(message);
	}

	public static PrintWriter log(String version) throws Exception {
		File dir = new File(System.getProperty("arctic.chatcopy.proof", "/home/claude/chatcopy-proof"));
		dir.mkdirs();
		return new PrintWriter(new FileWriter(new File(dir, version + "-results.txt")), true);
	}
}
