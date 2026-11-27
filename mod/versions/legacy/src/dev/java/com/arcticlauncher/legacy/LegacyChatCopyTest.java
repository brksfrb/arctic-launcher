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

	private static String hover(Object style) {
		net.minecraft.text.HoverEvent hover = ((Style) style).getHoverEvent();
		return hover == null ? "" : hover.getValue().asUnformattedString();
	}

	private static void checkHoverStyles() {
		net.minecraft.text.HoverEvent event = new net.minecraft.text.HoverEvent(net.minecraft.text.HoverEvent.Action.SHOW_TEXT, new LiteralText("server hover"));
		Style server = new Style().setInsertion("server insertion").setHoverEvent(event);
		Text body = new LiteralText("probe");
		body.setStyle(server);
		ClickEvent click = new ClickEvent(ClickEvent.Action.RUN_COMMAND, "/help");
		body.getStyle().setClickEvent(click);
		Style tagged = LegacyChatCopy.tag(body, "probe", false).getStyle();
		if (tagged.getClickEvent() != click || tagged.getHoverEvent() != event || !"server insertion".equals(tagged.getInsertion()))
			throw new AssertionError("body server click/hover/insertion changed");
		if (!hover(LegacyChatCopy.tag(new LiteralText("probe"), "probe", false).getStyle()).isEmpty())
			throw new AssertionError("plain body gained hover");

		Style existing = LegacyChatCopy.tag(new LiteralText("probe"), "probe", true).getStyle();
		if (!"Click to copy".equals(hover(existing)))
			throw new AssertionError("copy target missing hover");
		ArcticClient.config().chatCopy = false;
		if (!hover(existing).isEmpty())
			throw new AssertionError("existing enabled target retained hover after disabling");
		Style receivedDisabled = LegacyChatCopy.tag(new LiteralText("probe"), "probe", true).getStyle();
		if (!hover(receivedDisabled).isEmpty())
			throw new AssertionError("disabled target gained hover");
		if (tagged.getHoverEvent() != event)
			throw new AssertionError("disabled body lost server hover");
		ArcticClient.config().chatCopy = true;
		if (!"Click to copy".equals(hover(existing)) || !"Click to copy".equals(hover(receivedDisabled)))
			throw new AssertionError("existing targets missing hover after enabling");
		ArcticClient.config().chatOff = true;
		if (!hover(existing).isEmpty() || !hover(receivedDisabled).isEmpty())
			throw new AssertionError("hidden chat target retained hover");
		ArcticClient.config().chatOff = false;
		if (!"Click to copy".equals(hover(existing)) || !"Click to copy".equals(hover(receivedDisabled)))
			throw new AssertionError("existing targets missing hover after showing chat");
		if (tagged.getHoverEvent() != event)
			throw new AssertionError("body server hover changed after toggles");
	}

	private static void emoteMessages() {
		try { log = ChatCopyProof.log("1.8.9"); }
		catch (Exception error) { throw new RuntimeException(error); }
		ArcticClient.config().chatEmoji = true;
		ArcticClient.config().emojiButton = true;
		ArcticClient.config().chatTimestamps = true;
		ArcticClient.config().chatCopy = true;
		ArcticClient.config().chatStack = false;
		ArcticClient.config().chatKeep = true;
		ArcticClient.config().fancy = !Boolean.getBoolean("arctic.emotePixelFont");
		MinecraftClient mc = MinecraftClient.getInstance();
		mc.setScreen(new ChatScreen());
		add(new LiteralText(com.arcticlauncher.client.feature.EmoteProof.INCOMING));
		add(new LiteralText(com.arcticlauncher.client.feature.EmoteProof.WRAP));
		Text split = new LiteralText("Split :s").append(new LiteralText("ob: styled"));
		add(split);
		String layout = LegacyEmotes.layout(new LiteralText(":sob:")).asUnformattedString();
		if (layout.length() != 1 || mc.textRenderer.getStringWidth(layout) != 10) throw new AssertionError("sprite metrics");
		ArcticClient.platform().sendChat(com.arcticlauncher.client.feature.QuickMessages.fillTyped(":sob:"));
		log.println("PASS shortcode layout is one 10px glyph; sent literal :sob: to offline integrated server");
		later(4, () -> {
			com.arcticlauncher.client.feature.EmoteProof.check(new ChatCopyProof.Game() {
				public String hoverText(Object style) { return ""; }
				public void hoverAt(int x, int y) {}
				public boolean colored(int x, int y, Object style) { return false; }
				public String action(Object style) { return ""; }
				public int width() { return (int) new Window(mc).getScaledWidth(); }
				public int height() { return (int) new Window(mc).getScaledHeight(); }
				public Object hit(int x, int y) {
					int scale = new Window(mc).getScaleFactor();
					Text part = mc.inGameHud.getChatHud().getTextAt(x * scale, (height() - y) * scale);
					return part == null ? null : part.getStyle();
				}
				public boolean click(int button, int x, int y, boolean shift) {
					int scale = new Window(mc).getScaleFactor();
					return LegacyChatCopy.click(button, x * scale, (height() - y) * scale, shift);
				}
			}, log);
			shot("emotes-chat");
			Window window = new Window(mc);
			int width = (int) window.getScaledWidth(), height = (int) window.getScaledHeight();
			com.arcticlauncher.client.feature.EmojiPicker.click(ArcticClient.config(), width - 10, height - 8, width, height);
			later(2, () -> {
				shot("emotes-picker");
				later(2, () -> {
					com.arcticlauncher.client.feature.EmojiPicker.closed();
					ArcticClient.config().chatEmoji = false;
					later(2, () -> { shot("emotes-disabled"); finish(); });
				});
			});
		});
	}

	private static void phase(int phase) {
		if (Boolean.getBoolean("arctic.emoteProof")) { emoteMessages(); return; }
		try { if (log == null) log = ChatCopyProof.log("1.8.9"); }
		catch (Exception error) { throw new RuntimeException(error); }
		MinecraftClient mc = MinecraftClient.getInstance();
		ArcticClient.config().chatCopy = true;
		ArcticClient.config().chatTimestamps = phase % 2 == 0;
		ArcticClient.config().chatStack = true;
		ArcticClient.config().chatKeep = false;
		ArcticClient.config().chatOff = false;
		checkHoverStyles();
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
				public String hoverText(Object style) { return hover(style); }
				public void hoverAt(int x, int y) {
					org.lwjgl.input.Mouse.setCursorPosition(x * window.getScaleFactor(), (height() - y) * window.getScaleFactor());
				}
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
				public boolean colored(int x, int y, Object style) {
					// Legacy hit-testing returns the literal's style; inline codes are
					// interpreted by the renderer. Locate the green word by its advance.
					int left = x;
					while (left > 0) {
						com.arcticlauncher.client.feature.ChatCopy.Data data = com.arcticlauncher.client.feature.ChatCopy.data(hit(left - 1, y));
						if (data == null || data.target || !ChatCopyProof.SHORT_PLAIN.equals(data.text)) break;
						left--;
					}
					int start = left + mc.textRenderer.getStringWidth("Plain chat: café ");
					return x >= start + 2 && x < start + mc.textRenderer.getStringWidth("colored") - 2;
				}
				public String action(Object style) {
					return style == null || ((Style) style).getClickEvent() == null ? "" : ((Style) style).getClickEvent().getAction().name();
				}
			}, log, "phase=" + phase + " fancy=" + (phase < 2) + " timestamps=" + (phase % 2 == 0));
			later(1, () -> {
				shot("chat-" + phase);
				if (phase == 0) shot("hover");
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
