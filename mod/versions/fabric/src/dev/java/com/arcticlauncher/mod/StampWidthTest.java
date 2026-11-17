package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.ChatStamp;
import com.arcticlauncher.client.feature.ChatStampWidth;
import java.util.Calendar;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.ChatScreen;

/** -Darctic.selftest=stampwidth: chat lines stamped 11:11, 08:58, 20:00, 19:19 with no padding, left-aligned and right-aligned slots. */
final class StampWidthTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-stampwidth");
		t.setDaemon(true);
		return t;
	});
	private static final int[][] TIMES = {{11, 11}, {8, 58}, {20, 0}, {19, 19}, {12, 12}, {1, 1}};
	private static final String[] NAMES = {"before", "left", "right"};

	private StampWidthTest() {}

	static void start() {
		later(8, () -> WorldTest.createTestWorld(() -> {
			ArcticClient.config().chatTimestamps = true;
			ArcticClient.config().chatCopy = true;
			ArcticClient.config().chatStack = false;
			ArcticClient.config().chatStamp12h = false;
			ArcticClient.config().chatStampSeconds = false;
			ArcticClient.config().chatStampAmPm = false;
			ArcticClient.config().chatStampBrackets = true;
			ArcticClient.config().fancy = Boolean.parseBoolean(System.getProperty("arctic.stampwidth.fancy", "true"));
			com.arcticlauncher.client.gfx.Draw.fancy = ArcticClient.config().fancy;
			//#if MC >= 26.1
			ArcticPacks.setSmoothFont(ArcticClient.config().fancy);
			//#else
			SmoothFont.changed();
			//#endif
			for (int i = 0; i < NAMES.length; i++) {
				int mode = i;
				later(4 + i * 6, () -> scene(mode));
			}
			later(4 + NAMES.length * 6 + 2, () -> Minecraft.getInstance().stop());
		}));
	}

	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> Minecraft.getInstance().execute(action), seconds, TimeUnit.SECONDS);
	}

	private static void scene(int mode) {
		Minecraft mc = Minecraft.getInstance();
		ChatStampWidth.proofOff = mode == 0;
		ChatStampWidth.proofRight = mode == 2;
		//#if MC >= 26.2
		net.minecraft.client.gui.components.ChatComponent chat = mc.gui.hud.getChat();
		//#else
		net.minecraft.client.gui.components.ChatComponent chat = mc.gui.getChat();
		//#endif
		chat.clearMessages(true);
		//#if MC >= 1.21.9
		Compat.setScreen(new ChatScreen("", false));
		//#else
		Compat.setScreen(new ChatScreen(""));
		//#endif
		String[] who = {"Steve", "Alex", "Notch", "Herobrine", "Dream", "Technoblade"};
		for (int i = 0; i < TIMES.length; i++) {
			Calendar cal = Calendar.getInstance();
			cal.set(Calendar.HOUR_OF_DAY, TIMES[i][0]);
			cal.set(Calendar.MINUTE, TIMES[i][1]);
			ChatStamp.proofNow = cal.getTime();
			chat.addMessage(Compat.literal("<" + who[i] + "> message number " + i + (i == 2 ? " which is long enough to wrap onto a second chat line so the continuation can be checked" : "")));
		}
		ChatStamp.proofNow = null;
		later(3, () -> {
			WorldTest.shot("stampwidth-" + NAMES[mode]);
			ArcticMod.LOG.info("stampwidth: shot {}", NAMES[mode]);
		});
	}
}
