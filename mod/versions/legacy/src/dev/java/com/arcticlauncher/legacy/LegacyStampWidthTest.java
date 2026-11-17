package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.ChatStamp;
import com.arcticlauncher.client.feature.ChatStampWidth;
import java.io.File;
import java.util.Calendar;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.hud.ChatHud;
import net.minecraft.client.gui.screen.ChatScreen;
import net.minecraft.text.LiteralText;

/** -Darctic.selftest=stampwidth: chat lines stamped 11:11, 08:58, 20:00, 19:19 with no padding, left-aligned and right-aligned slots. */
final class LegacyStampWidthTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-stampwidth");
		t.setDaemon(true);
		return t;
	});
	private static final int[][] TIMES = {{11, 11}, {8, 58}, {20, 0}, {19, 19}, {12, 12}, {1, 1}};
	private static final String[] NAMES = {"before", "left", "right"};

	private LegacyStampWidthTest() {}

	static void start() {
		later(8, () -> LegacyWorldTest.createTestWorld(() -> {
			ArcticClient.config().chatTimestamps = true;
			ArcticClient.config().chatCopy = true;
			ArcticClient.config().chatStack = false;
			ArcticClient.config().chatStamp12h = false;
			ArcticClient.config().chatStampSeconds = false;
			ArcticClient.config().chatStampAmPm = false;
			ArcticClient.config().chatStampBrackets = true;
			boolean fancy = Boolean.parseBoolean(System.getProperty("arctic.stampwidth.fancy", "true"));
			ArcticClient.config().fancy = fancy;
			com.arcticlauncher.client.gfx.Draw.fancy = fancy;
			for (int i = 0; i < NAMES.length; i++) {
				int mode = i;
				later(4 + i * 6, () -> scene(mode));
			}
			later(4 + NAMES.length * 6 + 2, () -> MinecraftClient.getInstance().scheduleStop());
		}));
	}

	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> LegacyHooks.onNextFrame(action), seconds, TimeUnit.SECONDS);
	}

	private static void scene(int mode) {
		MinecraftClient mc = MinecraftClient.getInstance();
		ChatStampWidth.proofOff = mode == 0;
		ChatStampWidth.proofRight = mode == 2;
		mc.setScreen(new ChatScreen());
		ChatHud chat = mc.inGameHud.getChatHud();
		chat.clear();
		String[] who = {"Steve", "Alex", "Notch", "Herobrine", "Dream", "Technoblade"};
		for (int i = 0; i < TIMES.length; i++) {
			Calendar cal = Calendar.getInstance();
			cal.set(Calendar.HOUR_OF_DAY, TIMES[i][0]);
			cal.set(Calendar.MINUTE, TIMES[i][1]);
			ChatStamp.proofNow = cal.getTime();
			chat.addMessage(new LiteralText("<" + who[i] + "> message number " + i + (i == 2 ? " which is long enough to wrap onto a second chat line so the continuation can be checked" : "")));
		}
		ChatStamp.proofNow = null;
		net.minecraft.client.font.TextRenderer font = mc.textRenderer;
		StringBuilder widths = new StringBuilder();
		for (String probe : new String[] {" ", "\u2009", "\u200a", "[11:11] ", "[08:58] ", "[20:00] ", "[19:19] ", "[00:00] ", "1", "0"}) {
			widths.append(probe.length() == 1 && probe.charAt(0) > 127 ? "U+" + Integer.toHexString(probe.charAt(0)) : probe).append('=').append(font.getStringWidth(probe)).append(' ');
		}
		ArcticLegacy.LOG.info("stampwidth: widths mode={} fancy={} {}", NAMES[mode], ArcticClient.config().fancy, widths);
		later(3, () -> {
			net.minecraft.client.util.ScreenshotUtils.saveScreenshot(mc.runDirectory, mc.width, mc.height, mc.getFramebuffer());
			File dir = new File(mc.runDirectory, "screenshots");
			File newest = null;
			File[] files = dir.listFiles();
			if (files != null) {
				for (File f : files) {
					if (f.getName().endsWith(".png") && (newest == null || f.lastModified() > newest.lastModified())) newest = f;
				}
			}
			if (newest != null) newest.renameTo(new File(dir, "stampwidth-" + NAMES[mode] + ".png"));
			ArcticLegacy.LOG.info("stampwidth: shot {}", NAMES[mode]);
		});
	}
}
