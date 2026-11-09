package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.ChatCopyProof;
import com.arcticlauncher.client.menu.Menus;
import java.io.PrintWriter;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.ChatScreen;
import net.minecraft.client.util.Window;
import net.minecraft.text.ClickEvent;
import net.minecraft.text.LiteralText;
import net.minecraft.text.Style;
import net.minecraft.text.Text;

final class LegacyChatCopyTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread thread = new Thread(r, "arctic-chatcopy-proof");
		thread.setDaemon(true);
		return thread;
	});
	private static PrintWriter log;
	private static int failed;

	static void start() { later(8, () -> LegacyWorldTest.createTestWorld(() -> phase(0))); }

	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> {
			try { action.run(); }
			catch (Throwable error) {
				failed++;
				if (log != null) { error.printStackTrace(log); log.flush(); }
				ArcticLegacy.LOG.error("CHAT_COPY FAILED", error);
				finish();
			}
		}), seconds, TimeUnit.SECONDS);
	}

	private static void phase(int phase) {
		try { if (log == null) log = ChatCopyProof.log("1.8.9"); }
		catch (Exception error) { throw new RuntimeException(error); }
		MinecraftClient mc = MinecraftClient.getInstance();
		ArcticClient.config().chatCopy = true;
		ArcticClient.config().chatTimestamps = phase % 2 == 0;
		ArcticClient.config().chatStack = true;
		ArcticClient.config().chatKeep = false;
		ArcticClient.config().chatOff = false;
		ArcticClient.config().fancy = phase < 2;
		com.arcticlauncher.client.gfx.Draw.fancy = phase < 2;
		//#if MC >= 1.11
		mc.inGameHud.getChatHud().clear(true);
		//#else
		mc.inGameHud.getChatHud().clear();
		//#endif
		mc.setScreen(new ChatScreen());
		add(new LiteralText(ChatCopyProof.SHORT));
		Text url = new LiteralText(ChatCopyProof.URL);
		url.setStyle(new Style().setClickEvent(new ClickEvent(ClickEvent.Action.OPEN_URL, "https://example.com/test?q=chat")));
		add(url);
		Text run = new LiteralText(ChatCopyProof.RUN);
		run.setStyle(new Style().setClickEvent(new ClickEvent(ClickEvent.Action.RUN_COMMAND, "/help")));
		add(run);
		Text suggest = new LiteralText(ChatCopyProof.SUGGEST);
		suggest.setStyle(new Style().setClickEvent(new ClickEvent(ClickEvent.Action.SUGGEST_COMMAND, "/help")));
		add(suggest);
		add(new LiteralText(ChatCopyProof.WRAP));
		add(new LiteralText(ChatCopyProof.REPEAT));
		add(new LiteralText(ChatCopyProof.REPEAT));
		later(2, () -> {
			Window window = new Window(mc);
			ChatCopyProof.check(new ChatCopyProof.Game() {
				public int width() { return (int) window.getScaledWidth(); }
				public int height() { return (int) window.getScaledHeight(); }
				public Object hit(int x, int y) {
					Text part = mc.inGameHud.getChatHud().getTextAt(rawX(x), rawY(y));
					return part == null ? null : part.getStyle();
				}
				public boolean click(int button, int x, int y, boolean shift) {
					return LegacyChatCopy.click(button, rawX(x), rawY(y), shift);
				}
				private int rawX(int x) { return x * window.getScaleFactor(); }
				private int rawY(int y) { return (height() - y) * window.getScaleFactor(); }
				public String action(Object style) {
					return style == null || ((Style) style).getClickEvent() == null ? "" : ((Style) style).getClickEvent().getAction().name();
				}
			}, log, "phase=" + phase + " fancy=" + (phase < 2) + " timestamps=" + (phase % 2 == 0));
			later(1, () -> {
				shot("chat-" + phase);
				if (phase < 3) phase(phase + 1);
				else {
					Menus.select("chat");
					ArcticClient.platform().openPage(Menus.selected());
					later(7, () -> { shot("settings"); finish(); });
				}
			});
		});
	}

	private static void add(Text text) { MinecraftClient.getInstance().inGameHud.getChatHud().addMessage(text); }

	private static void shot(String name) {
		MinecraftClient mc = MinecraftClient.getInstance();
		net.minecraft.client.util.ScreenshotUtils.saveScreenshot(mc.runDirectory, mc.width, mc.height, mc.getFramebuffer());
		try {
			java.io.File newest = java.util.Arrays.stream(new java.io.File(mc.runDirectory, "screenshots").listFiles())
					.filter(file -> file.getName().endsWith(".png")).max(java.util.Comparator.comparingLong(java.io.File::lastModified)).get();
			java.nio.file.Files.copy(newest.toPath(), java.nio.file.Paths.get(System.getProperty("arctic.chatcopy.proof",
					"/home/claude/chatcopy-proof"), "1.8.9-" + name + ".png"), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
		} catch (Exception error) { throw new RuntimeException(error); }
	}

	private static void finish() {
		if (log != null) { log.println("CHAT_COPY " + (failed == 0 ? "OK" : "FAILED")); log.close(); }
		later(2, () -> Runtime.getRuntime().halt(failed == 0 ? 0 : 1));
	}
}
