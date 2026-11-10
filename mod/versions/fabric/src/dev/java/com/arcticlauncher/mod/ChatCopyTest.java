package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.ChatCopyProof;
import com.arcticlauncher.client.feature.ChatCopy;
import com.arcticlauncher.client.menu.Menus;
import java.io.PrintWriter;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.network.chat.Style;

/** -Darctic.selftest=chatcopy: offline world, actual chat hit-tests and OS clipboard readback. */
final class ChatCopyTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread thread = new Thread(r, "arctic-chatcopy-proof");
		thread.setDaemon(true);
		return thread;
	});
	private static PrintWriter log;
	private static int failed;
	private static final String VERSION = System.getProperty("arctic.chatcopy.version", "fabric");

	static void start() {
		later(8, () -> WorldTest.createTestWorld(() -> phase(Integer.getInteger("arctic.chatcopy.startPhase", 0))));
	}

	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> Minecraft.getInstance().execute(() -> {
			try { action.run(); }
			catch (Throwable error) {
				failed++;
				if (log != null) { error.printStackTrace(log); log.flush(); }
				ArcticMod.LOG.error("CHAT_COPY FAILED", error);
				finish();
			}
		}), seconds, TimeUnit.SECONDS);
	}

	private static ChatComponent chat() {
		//#if MC >= 26.2
		return Minecraft.getInstance().gui.hud.getChat();
		//#else
		return Minecraft.getInstance().gui.getChat();
		//#endif
	}

	private static String hover(Object style) {
		net.minecraft.network.chat.HoverEvent hover = ((Style) style).getHoverEvent();
		if (hover == null) return "";
		//#if MC >= 1.21.5
		return hover instanceof net.minecraft.network.chat.HoverEvent.ShowText ? ((net.minecraft.network.chat.HoverEvent.ShowText) hover).value().getString() : "";
		//#elif MC >= 1.16
		return hover.getValue(net.minecraft.network.chat.HoverEvent.Action.SHOW_TEXT).getString();
		//#else
		return hover.getValue().getString();
		//#endif
	}

	private static void checkHoverStyles() {
		net.minecraft.network.chat.HoverEvent event = null;
		//#if MC >= 1.21.5
		event = new net.minecraft.network.chat.HoverEvent.ShowText(Compat.literal("server hover"));
		//#else
		event = new net.minecraft.network.chat.HoverEvent(net.minecraft.network.chat.HoverEvent.Action.SHOW_TEXT, Compat.literal("server hover"));
		//#endif
		Style server = null;
		//#if MC >= 1.16
		server = Style.EMPTY.withInsertion("server insertion").withHoverEvent(event);
		//#else
		server = new Style().setInsertion("server insertion").setHoverEvent(event);
		//#endif
		net.minecraft.network.chat.MutableComponent body = Compat.literal("probe");
		body.setStyle(server);
		net.minecraft.network.chat.ClickEvent click = null;
		//#if MC >= 1.21.5
		click = new net.minecraft.network.chat.ClickEvent.RunCommand("/help");
		//#else
		click = new net.minecraft.network.chat.ClickEvent(net.minecraft.network.chat.ClickEvent.Action.RUN_COMMAND, "/help");
		//#endif
		//#if MC >= 1.16
		body.setStyle(body.getStyle().withClickEvent(click));
		//#else
		body.getStyle().setClickEvent(click);
		//#endif
		Style tagged = ChatCopyComponents.tag(body, "probe", false).getStyle();
		if (tagged.getClickEvent() != click || tagged.getHoverEvent() != event || !"server insertion".equals(tagged.getInsertion()))
			throw new AssertionError("body server click/hover/insertion changed");
		if (!hover(ChatCopyComponents.tag(Compat.literal("probe"), "probe", false).getStyle()).isEmpty())
			throw new AssertionError("plain body gained hover");
		if (!"Click to copy".equals(hover(ChatCopyComponents.tag(Compat.literal("probe"), "probe", true).getStyle())))
			throw new AssertionError("copy target missing hover");
		ArcticClient.config().chatCopy = false;
		if (!hover(ChatCopyComponents.tag(Compat.literal("probe"), "probe", true).getStyle()).isEmpty())
			throw new AssertionError("disabled target gained hover");
		ArcticClient.config().chatCopy = true;
		ArcticClient.config().chatOff = true;
		if (!hover(ChatCopyComponents.tag(Compat.literal("probe"), "probe", true).getStyle()).isEmpty())
			throw new AssertionError("hidden chat target gained hover");
		ArcticClient.config().chatOff = false;
	}

	private static void phase(int phase) {
		try { if (log == null) log = ChatCopyProof.log(VERSION); }
		catch (Exception error) { throw new RuntimeException(error); }
		//#if MC >= 1.16
		ChatCopy.Data owner = new ChatCopy.Data("style transformation regression", false);
		Style original = Style.EMPTY.withInsertion("owned");
		((ChatCopy.Tagged) (Object) original).arctic$copyData(owner);
		for (Style changed : new Style[] {
				original.withColor(net.minecraft.ChatFormatting.GREEN),
				original.withBold(true).withItalic(true).withUnderlined(true),
				original.withClickEvent(null).withHoverEvent(null).withFont(null),
				original.withInsertion(null),
				original.applyFormat(net.minecraft.ChatFormatting.BOLD),
				original.applyLegacyFormat(net.minecraft.ChatFormatting.GREEN),
				original.applyLegacyFormat(net.minecraft.ChatFormatting.RESET),
				original.applyFormats(net.minecraft.ChatFormatting.GREEN, net.minecraft.ChatFormatting.RESET),
				original.applyTo(Style.EMPTY)}) {
			if (!owner.equals(ChatCopy.data(changed))) throw new AssertionError("style lost ownership");
		}
		if (ChatCopy.data(Style.EMPTY) != null) throw new AssertionError("global EMPTY was tagged");
		//#endif
		ArcticClient.config().chatCopy = true;
		ArcticClient.config().chatTimestamps = phase % 2 == 0;
		ArcticClient.config().chatStack = true;
		ArcticClient.config().chatKeep = false;
		ArcticClient.config().chatOff = false;
		checkHoverStyles();
		ArcticClient.config().fancy = phase < 2;
		com.arcticlauncher.client.gfx.Draw.fancy = phase < 2;
		//#if MC >= 26.1
		ArcticPacks.setSmoothFont(phase < 2);
		//#else
		SmoothFont.changed();
		//#endif
		later(4, () -> messages(phase));
	}

	private static void messages(int phase) {
		chat().clearMessages(true);
		//#if MC >= 1.21.9
		Compat.setScreen(new ChatScreen("", false));
		//#else
		Compat.setScreen(new ChatScreen(""));
		//#endif
		add(ChatCopyProof.SHORT);
		//#if MC >= 1.21.5
		net.minecraft.network.chat.ClickEvent event = new net.minecraft.network.chat.ClickEvent.OpenUrl(java.net.URI.create("https://example.com/test?q=chat"));
		//#else
		net.minecraft.network.chat.ClickEvent event = new net.minecraft.network.chat.ClickEvent(net.minecraft.network.chat.ClickEvent.Action.OPEN_URL, "https://example.com/test?q=chat");
		//#endif
		//#if MC >= 1.16
		add(Compat.literal(ChatCopyProof.URL).withStyle(style -> style.withClickEvent(event)));
		//#else
		net.minecraft.network.chat.Component url = Compat.literal(ChatCopyProof.URL);
		url.setStyle(new Style().setClickEvent(event));
		add(url);
		//#endif
		//#if MC >= 1.21.5
		add(Compat.literal(ChatCopyProof.RUN).withStyle(style -> style.withClickEvent(new net.minecraft.network.chat.ClickEvent.RunCommand("/help"))));
		add(Compat.literal(ChatCopyProof.SUGGEST).withStyle(style -> style.withClickEvent(new net.minecraft.network.chat.ClickEvent.SuggestCommand("/help"))));
		//#else
		command(ChatCopyProof.RUN, net.minecraft.network.chat.ClickEvent.Action.RUN_COMMAND);
		command(ChatCopyProof.SUGGEST, net.minecraft.network.chat.ClickEvent.Action.SUGGEST_COMMAND);
		//#endif
		add(ChatCopyProof.WRAP);
		add(ChatCopyProof.REPEAT);
		add(ChatCopyProof.REPEAT);
		later(2, () -> {
			Minecraft mc = Minecraft.getInstance();
			ChatCopyProof.check(new ChatCopyProof.Game() {
				public String hoverText(Object style) { return hover(style); }
				public void hoverAt(int x, int y) {
					// Use the controller's native cursor adapter, which also reports GLFW warps to Minecraft.
					ArcticClient.platform().setCursor(x * mc.getWindow().getGuiScale(), y * mc.getWindow().getGuiScale());
					ArcticClient.platform().setCursor(x * mc.getWindow().getGuiScale(), y * mc.getWindow().getGuiScale());
				}
				public int width() { return mc.getWindow().getGuiScaledWidth(); }
				public int height() { return mc.getWindow().getGuiScaledHeight(); }
				public Object hit(int x, int y) { return ChatCopyInput.hit(x, y); }
				public boolean click(int button, int x, int y, boolean shift) { return ChatCopyInput.click(button, x, y, shift); }
				public boolean colored(int x, int y, Object style) {
					//#if MC >= 1.16
					return style != null && ((Style) style).getColor() != null && ((Style) style).getColor().getValue() == 0x55ff55;
					//#else
					return style != null && ((Style) style).getColor() == net.minecraft.ChatFormatting.GREEN;
					//#endif
				}
				public String action(Object style) {
					if (style == null || ((Style) style).getClickEvent() == null) return "";
					//#if MC >= 1.21.5
					return ((Style) style).getClickEvent().action().name();
					//#else
					return ((Style) style).getClickEvent().getAction().name();
					//#endif
				}
			}, log, "phase=" + phase + " fancy=" + (phase < 2) + " timestamps=" + (phase % 2 == 0));
			later(1, () -> {
				shot("chat-" + phase);
				if (phase == 0) shot("hover");
				if (phase < Integer.getInteger("arctic.chatcopy.endPhase", 3)) phase(phase + 1);
				else {
					Menus.select("chat");
					ArcticClient.platform().openPage(Menus.selected());
					later(7, () -> { shot("settings"); finish(); });
				}
			});
		});
	}

	//#if MC < 1.21.5
	private static void command(String text, net.minecraft.network.chat.ClickEvent.Action action) {
		net.minecraft.network.chat.ClickEvent event = new net.minecraft.network.chat.ClickEvent(action, "/help");
		//#if MC >= 1.16
		add(Compat.literal(text).withStyle(style -> style.withClickEvent(event)));
		//#else
		net.minecraft.network.chat.Component message = Compat.literal(text);
		message.setStyle(new Style().setClickEvent(event));
		add(message);
		//#endif
	}
	//#endif

	private static void add(String text) { add(Compat.literal(text)); }
	private static void add(net.minecraft.network.chat.Component text) {
		//#if MC >= 26.1
		chat().addClientSystemMessage(text);
		//#else
		chat().addMessage(text);
		//#endif
	}

	private static void shot(String name) {
		WorldTest.shot(name);
		later(1, () -> {
			try {
				java.nio.file.Path newest;
				try (java.util.stream.Stream<java.nio.file.Path> files = java.nio.file.Files.list(
						Minecraft.getInstance().gameDirectory.toPath().resolve("screenshots"))) {
					newest = files.filter(path -> path.toString().endsWith(".png"))
							.max(java.util.Comparator.comparingLong(path -> path.toFile().lastModified())).get();
				}
				java.nio.file.Files.copy(newest, java.nio.file.Paths.get(System.getProperty("arctic.chatcopy.proof",
						"/home/claude/chatcopy-proof"), VERSION + "-" + name + ".png"), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
			} catch (Exception error) { throw new RuntimeException(error); }
		});
	}

	private static void finish() {
		if (log != null) { log.println("CHAT_COPY " + (failed == 0 ? "OK" : "FAILED")); log.close(); }
		later(1, () -> Runtime.getRuntime().halt(failed == 0 ? 0 : 1));
	}
}
