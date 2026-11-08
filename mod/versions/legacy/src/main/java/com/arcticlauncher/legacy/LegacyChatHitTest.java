package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import java.io.File;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.hud.ChatHud;
import net.minecraft.client.gui.hud.ChatHudLine;
import net.minecraft.client.gui.screen.ChatScreen;
import net.minecraft.client.option.GameOptions;
import net.minecraft.client.util.ScreenshotUtils;
import net.minecraft.client.util.Window;
import net.minecraft.text.ClickEvent;
import net.minecraft.text.HoverEvent;
import net.minecraft.text.LiteralText;
import net.minecraft.text.Text;
import com.arcticlauncher.legacy.mixin.MinecraftClientResizeInvoker;

/**
 * {@code -Darctic.selftest=chathit}: a creative world with cheats, clickable {@code /tellraw}
 * lines, then the real {@link ChatHud#getTextAt} at the centre and the inset edges of each
 * component. Fancy on and off, several GUI scales and chat scales. The first pass forces the old
 * metrics ({@code -Darctic.chatHitOld}) so the miss is on record; the rest must hit.
 */
final class LegacyChatHitTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-chathit");
		t.setDaemon(true);
		return t;
	});

	private static final String LINE = "/tellraw @a {\"text\":\"\",\"extra\":["
			+ "{\"text\":\"[REPORT]\",\"clickEvent\":{\"action\":\"run_command\",\"value\":\"/say hit-report\"},"
			+ "\"hoverEvent\":{\"action\":\"show_text\",\"value\":{\"text\":\"report this\"}}},"
			+ "{\"text\":\" \"},"
			+ "{\"text\":\"[MUTE]\",\"clickEvent\":{\"action\":\"suggest_command\",\"value\":\"/mute Player\"},"
			+ "\"hoverEvent\":{\"action\":\"show_text\",\"value\":{\"text\":\"mute them\"}}}]}";
	private static final String ABOVE = "/tellraw @a {\"text\":\"[ABOVE]\","
			+ "\"clickEvent\":{\"action\":\"run_command\",\"value\":\"/say hit-above\"},"
			+ "\"hoverEvent\":{\"action\":\"show_text\",\"value\":{\"text\":\"the line above\"}}}";
	private static final String WRAP = "/tellraw @a {\"text\":\"wrap-start \",\"extra\":["
			+ "{\"text\":\"[ALPHA]\",\"clickEvent\":{\"action\":\"run_command\",\"value\":\"/say hit-alpha\"}},"
			+ "{\"text\":\" lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore \"},"
			+ "{\"text\":\"[OMEGA]\",\"clickEvent\":{\"action\":\"run_command\",\"value\":\"/say hit-omega\"}}]}";

	private static PrintWriter log;
	private static int failed;
	private static int passed;
	private static boolean sawOldMiss;

	private LegacyChatHitTest() {}

	static boolean requested() {
		return "chathit".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticLegacy.LOG.info("chathit: scheduled");
		TIMER.schedule(() -> run(() -> LegacyWorldTest.createTestWorld(LegacyChatHitTest::world)), 8, TimeUnit.SECONDS);
	}

	private static void run(Runnable r) {
		LegacyHooks.onNextFrame(r);
	}

	private static void world() {
		MinecraftClient mc = MinecraftClient.getInstance();
		try {
			log = new PrintWriter(new FileWriter(new File(mc.runDirectory, "chat-hit-log.txt")), true);
		} catch (Exception e) {
			ArcticLegacy.LOG.error("chathit: no log", e);
			mc.scheduleStop();
			return;
		}
		mc.options.pauseOnLostFocus = false;
		note("world ready");
		// Above first, so the report line is the newest (the bottom row). The chat packet is capped at
		// 100 characters, which cuts a tellraw in half, so the command runs on the integrated server.
		tellraw(ABOVE);
		tellraw(WRAP);
		tellraw(LINE);
		TIMER.schedule(() -> run(LegacyChatHitTest::configured), 2, TimeUnit.SECONDS);
	}

	/** One row of the matrix. {@code old} is the pre-fix metrics, recorded but not required to pass. */
	private static final class Probe {
		final String name;
		final int gui;
		final float chat;
		final float width;
		final boolean fancy;
		final boolean old;
		final boolean scroll;
		final boolean shot;

		Probe(String name, int gui, float chat, float width, boolean fancy, boolean old, boolean scroll, boolean shot) {
			this.name = name;
			this.gui = gui;
			this.chat = chat;
			this.width = width;
			this.fancy = fancy;
			this.old = old;
			this.scroll = scroll;
			this.shot = shot;
		}
	}

	private static void configured() {
		List<Probe> probes = new ArrayList<Probe>();
		probes.add(new Probe("before-fancy-gui2-chat1", 2, 1f, 1f, true, true, false, true));
		int[] guis = {1, 2, 3, 0};
		float[] chats = {0.5f, 0.75f, 1f};
		for (int gui : guis) {
			for (float chat : chats) {
				boolean shot = gui == 2 && chat == 1f || gui == 1 && chat == 0.5f || gui == 0 && chat == 0.75f;
				probes.add(new Probe("fancy-gui" + gui + "-chat" + chat, gui, chat, 1f, true, false, false, shot));
			}
		}
		probes.add(new Probe("fancy-narrow-wrap", 2, 1f, 0.5f, true, false, false, true));
		probes.add(new Probe("fancy-scrolled", 2, 1f, 1f, true, false, true, false));
		probes.add(new Probe("vanilla-gui2-chat1", 2, 1f, 1f, false, false, false, true));
		probes.add(new Probe("vanilla-gui1-chat0.5", 1, 0.5f, 1f, false, false, false, false));
		next(probes, 0);
	}

	private static void next(final List<Probe> probes, final int at) {
		if (at >= probes.size()) {
			finish();
			return;
		}
		final Probe probe = probes.get(at);
		run(() -> {
			try {
				apply(probe);
			} catch (Exception e) {
				fail(probe.name, "apply " + e);
				ArcticLegacy.LOG.error("chathit apply", e);
			}
		});
		// The scale change and the line rebuild land on the next frames.
		TIMER.schedule(() -> run(() -> {
			try {
				check(probe);
			} catch (Exception e) {
				fail(probe.name, "check " + e);
				ArcticLegacy.LOG.error("chathit check", e);
			}
			if (probe.shot) {
				shot(probe.name);
			}
			next(probes, at + 1);
		}), 800, TimeUnit.MILLISECONDS);
	}

	/** {@code /tellraw} on the integrated server (the same command a player sends, without the chat length cap). */
	private static void tellraw(String command) {
		MinecraftClient mc = MinecraftClient.getInstance();
		String cmd = command.startsWith("/") ? command.substring(1) : command;
		net.minecraft.server.MinecraftServer server = mc.getServer();
		net.minecraft.entity.player.ServerPlayerEntity player = server.getPlayerManager().getPlayer(mc.player.getUuid());
		int result = server.getCommandManager().execute(player, cmd);
		note("command " + result + " " + cmd.substring(0, Math.min(90, cmd.length())));
	}

	private static void apply(Probe probe) {
		MinecraftClient mc = MinecraftClient.getInstance();
		GameOptions options = mc.options;
		options.guiScale = probe.gui;
		options.chatScale = probe.chat;
		options.chatWidth = probe.width;
		// A full-height chat fits every test line, so the scroll probe uses a short one.
		options.chatHeightFocused = probe.scroll ? 0.05f : 1f;
		options.chatLink = true;
		ArcticClient.config().fancy = probe.fancy;
		com.arcticlauncher.client.gfx.Draw.fancy = probe.fancy;
		if (probe.old) {
			System.setProperty("arctic.chatHitOld", "1");
		} else {
			System.clearProperty("arctic.chatHitOld");
		}
		((MinecraftClientResizeInvoker) mc).arctic$resize(mc.width, mc.height);
		mc.setScreen(new ChatScreen());
		ChatHud chat = mc.inGameHud.getChatHud();
		chat.reset();
		if (probe.scroll) {
			chat.scroll(2);
		}
	}

	private static void check(Probe probe) {
		MinecraftClient mc = MinecraftClient.getInstance();
		Window window = new Window(mc);
		int gui = window.getScaleFactor();
		double scaledHeight = window.getScaledHeight();
		ChatHud chat = mc.inGameHud.getChatHud();
		int scroll = scrollOf(chat);
		int lineHeight = LegacySmoothFont.aligned() ? LegacySmoothFont.lineHeight() : mc.textRenderer.fontHeight;
		List<ChatHudLine> lines = linesOf(chat);
		note(probe.name + " guiScale=" + gui + " (option " + probe.gui + ") chatScale=" + probe.chat
				+ " fancy=" + probe.fancy + " old=" + probe.old + " lineHeight=" + lineHeight
				+ " scroll=" + scroll + " lines=" + lines.size());
		for (int i = 0; i < lines.size(); i++) {
			StringBuilder text = new StringBuilder();
			for (Text part : parts(lines.get(i).getText())) {
				if (part instanceof LiteralText) {
					text.append(((LiteralText) part).getRawString());
				}
			}
			note("  line " + i + " " + text);
		}
		if (probe.scroll && scroll == 0) {
			fail(probe.name, "chat did not scroll");
		}
		int hits = 0;
		for (int index = 0; index < lines.size(); index++) {
			int row = index - scroll;
			if (row < 0 || row >= chat.getVisibleLineCount()) {
				continue;
			}
			int start = 0;
			for (Text part : parts(lines.get(index).getText())) {
				if (!(part instanceof LiteralText)) {
					continue;
				}
				String raw = ((LiteralText) part).getRawString();
				int width = renderedWidth(mc, raw);
				if (part.getStyle() == null || part.getStyle().getClickEvent() == null || width <= 0) {
					start += width;
					continue;
				}
				hits++;
				String label = raw;
				ClickEvent click = part.getStyle().getClickEvent();
				HoverEvent hover = part.getStyle().getHoverEvent();
				int centre = sample(start, start + width, start + width / 2, probe.chat, gui, 2);
				int chatY = sample(row * lineHeight, (row + 1) * lineHeight, row * lineHeight + lineHeight / 2, probe.chat, gui, 28);
				String atCentre = hit(chat, probe, centre, chatY, gui);
				boolean centreOk = label.equals(atCentre);
				String atLeft = hit(chat, probe, firstPixel(start, start + width, probe.chat, gui), chatY, gui);
				String atRight = hit(chat, probe, lastPixel(start, start + width, probe.chat, gui), chatY, gui);
				// Vanilla's click origin is one pixel off the translate. The aligned path uses the
				// render translate, so the window pixels on the component must all be that component.
				boolean edgesOk = !LegacySmoothFont.aligned() || (label.equals(atLeft) && label.equals(atRight));
				String hoverText = hover == null || hover.getValue() == null ? "" : hover.getValue().asUnformattedString();
				String line = probe.name + " '" + label + "' click=" + click.getValue()
						+ " hover=" + hoverText
						+ " x=" + start + ".." + (start + width)
						+ " centre->" + atCentre + " left->" + atLeft + " right->" + atRight;
				if (centreOk && edgesOk) {
					pass(line);
				} else if (probe.old) {
					sawOldMiss = true;
					note("OLD MISS " + line);
				} else {
					fail(probe.name, line);
				}
				// The top of the bottom line used to land on the line above. Only the aligned path
				// promises that pixel; vanilla's origin is the one pixel it has always been.
				if ("[REPORT]".equals(label) && row == 0) {
					int topY = -1;
					for (int y = (row + 1) * lineHeight - 1; y >= row * lineHeight; y--) {
						if (windowPixel(y, probe.chat, gui, 28) >= 0) {
							topY = y;
							break;
						}
					}
					String top = topY < 0 ? "no-pixel" : hit(chat, probe, centre, topY, gui);
					note(probe.name + " top-of-report -> " + top);
					if (LegacySmoothFont.aligned() && !label.equals(top)) {
						fail(probe.name, "top of [REPORT] hit '" + top + "'");
					} else if (!LegacySmoothFont.aligned() && !label.equals(top)) {
						sawOldMiss = true;
						note("OLD TOP " + probe.name + " -> " + top);
					}
				}
				start += width;
			}
		}
		if (hits == 0) {
			fail(probe.name, "no clickable components were on screen");
		}
	}

	/** A chat coordinate in {@code [from, to)} that a window pixel lands on, preferring {@code prefer}. */
	private static int sample(int from, int to, int prefer, float chatScale, int gui, int origin) {
		if (from >= to) {
			return from;
		}
		prefer = Math.max(from, Math.min(to - 1, prefer));
		if (windowPixel(prefer, chatScale, gui, origin) >= 0) {
			return prefer;
		}
		for (int d = 1; d < to - from; d++) {
			if (prefer + d < to && windowPixel(prefer + d, chatScale, gui, origin) >= 0) {
				return prefer + d;
			}
			if (prefer - d >= from && windowPixel(prefer - d, chatScale, gui, origin) >= 0) {
				return prefer - d;
			}
		}
		return prefer;
	}

	/** First chat-local x in {@code [from, to)} that a real window pixel lands on, or {@code from}. */
	private static int firstPixel(int from, int to, float chatScale, int gui) {
		for (int x = from; x < to; x++) {
			if (windowPixel(x, chatScale, gui, 2) >= 0) {
				return x;
			}
		}
		return from;
	}

	private static int lastPixel(int from, int to, float chatScale, int gui) {
		for (int x = to - 1; x >= from; x--) {
			if (windowPixel(x, chatScale, gui, 2) >= 0) {
				return x;
			}
		}
		return Math.max(from, to - 1);
	}

	/**
	 * A window pixel whose chat hit-test (origin 2 across, 28 up, then the chat scale) is exactly
	 * {@code chat}, or -1 when the scale leaves that chat pixel between window pixels.
	 */
	private static int windowPixel(int chat, float chatScale, int gui, int origin) {
		double start = (chat * chatScale + origin) * gui;
		double end = ((chat + 1) * chatScale + origin) * gui;
		int mouse = (int) Math.ceil(start - 1e-6);
		if (mouse < end) {
			return mouse;
		}
		return -1;
	}

	private static String hit(ChatHud chat, Probe probe, int chatX, int chatY, int gui) {
		int mouseX = windowPixel(chatX, probe.chat, gui, 2);
		int mouseY = windowPixel(chatY, probe.chat, gui, 28);
		if (mouseX < 0 || mouseY < 0) {
			return "no-pixel";
		}
		Text hit = chat.getTextAt(mouseX, mouseY);
		if (!(hit instanceof LiteralText)) {
			return hit == null ? "null" : hit.getClass().getSimpleName();
		}
		return ((LiteralText) hit).getRawString();
	}

	/** The width the pen moves for this raw string, which is what is on screen. */
	private static int renderedWidth(MinecraftClient mc, String raw) {
		if (LegacySmoothFont.aligned() || !LegacySmoothFont.on()) {
			return mc.textRenderer.getStringWidth(raw);
		}
		int pen = 0;
		for (int i = 0; i < raw.length(); i++) {
			float advance = LegacySmoothFont.advance(raw.charAt(i));
			pen += advance < 0 ? Math.max(0, mc.textRenderer.getCharWidth(raw.charAt(i))) : (int) advance;
		}
		return pen;
	}

	@SuppressWarnings("unchecked")
	private static List<ChatHudLine> linesOf(ChatHud chat) {
		try {
			java.lang.reflect.Field field = ChatHud.class.getDeclaredField("visibleMessages");
			field.setAccessible(true);
			return (List<ChatHudLine>) field.get(chat);
		} catch (Exception e) {
			throw new IllegalStateException(e);
		}
	}

	private static int scrollOf(ChatHud chat) {
		try {
			java.lang.reflect.Field field = ChatHud.class.getDeclaredField("scrolledLines");
			field.setAccessible(true);
			return field.getInt(chat);
		} catch (Exception e) {
			throw new IllegalStateException(e);
		}
	}

	/** Depth-first, the component itself then its siblings, as the game's iterator does. */
	private static List<Text> parts(Text text) {
		List<Text> out = new ArrayList<Text>();
		walk(text, out);
		return out;
	}

	@SuppressWarnings("unchecked")
	private static void walk(Text text, List<Text> out) {
		if (text instanceof LiteralText) {
			out.add(text);
		}
		for (Object sibling : text.getSiblings()) {
			walk((Text) sibling, out);
		}
	}

	private static void shot(String name) {
		MinecraftClient mc = MinecraftClient.getInstance();
		ScreenshotUtils.saveScreenshot(mc.runDirectory, mc.width, mc.height, mc.getFramebuffer());
		File dir = new File(mc.runDirectory, "screenshots");
		File newest = null;
		File[] files = dir.listFiles();
		if (files != null) {
			for (File file : files) {
				if (file.getName().endsWith(".png") && (newest == null || file.lastModified() > newest.lastModified())) {
					newest = file;
				}
			}
		}
		if (newest != null) {
			File named = new File(dir, "chat-hit-" + name + ".png");
			try {
				java.nio.file.Files.copy(newest.toPath(), named.toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
				File artifacts = new File("/opt/cursor/artifacts");
				artifacts.mkdirs();
				java.nio.file.Files.copy(named.toPath(), new File(artifacts, named.getName()).toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
			} catch (Exception e) {
				note("screenshot copy failed " + e);
			}
		}
		note("screenshot " + name);
	}

	private static void pass(String line) {
		passed++;
		note("PASS " + line);
	}

	private static void fail(String name, String line) {
		failed++;
		note("FAIL " + name + " " + line);
		ArcticLegacy.LOG.error("chathit FAIL {} {}", name, line);
	}

	private static void note(String line) {
		System.out.println("chathit " + line);
		ArcticLegacy.LOG.info("chathit {}", line);
		if (log != null) {
			log.println(line);
			log.flush();
		}
	}

	private static void finish() {
		note("old-metrics-missed=" + sawOldMiss + " passed=" + passed + " failed=" + failed);
		note(failed == 0 ? "CHAT_HIT OK" : "CHAT_HIT FAILED");
		if (log != null) {
			log.close();
		}
		try {
			File artifacts = new File("/opt/cursor/artifacts");
			artifacts.mkdirs();
			java.nio.file.Files.copy(new File(MinecraftClient.getInstance().runDirectory, "chat-hit-log.txt").toPath(),
					new File(artifacts, "chat-hit-189-log.txt").toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
		} catch (Exception ignored) {
			// The run directory still has the log.
		}
		TIMER.schedule(() -> run(() -> {
			MinecraftClient.getInstance().scheduleStop();
			TIMER.schedule(() -> Runtime.getRuntime().halt(failed == 0 ? 0 : 1), 2, TimeUnit.SECONDS);
		}), 1, TimeUnit.SECONDS);
	}
}
