package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.hud.VanillaParts;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.util.Window;

/**
 * The scoreboard and boss bar drawn where the HUD editor put them, on 1.8.9 to 1.12.2 (as
 * VanillaMove on newer versions): the game draws them inside a move of the drawing, or not at all.
 */
public final class LegacyVanillaMove {
	private static int[] scoreboard;
	private static boolean capturing;
	private static long capStarted;
	private static final long CAPTURE_NANOS = 5_000_000L;
	private static int capX0;
	private static int capY0;
	private static int capX1;
	private static int capY1;
	private static boolean scoreboardMoved;
	private static boolean bossMoved;

	private LegacyVanillaMove() {}

	/** Before the scoreboard is drawn; false: don't draw it. */
	public static boolean scoreboardBegin() {
		capturing = true;
		capStarted = System.nanoTime();
		capX0 = Integer.MAX_VALUE;
		capY0 = Integer.MAX_VALUE;
		capX1 = Integer.MIN_VALUE;
		capY1 = Integer.MIN_VALUE;
		float[] p = scoreboard == null ? null
				: ArcticClient.hud().vanillaPlacement(VanillaParts.SCOREBOARD, scoreboard[0], scoreboard[1], scoreboard[2], scoreboard[3]);
		// The game keeps its scoreboard against the right edge and halfway down, so only those stay put when
		// its lines change length: moving by the box's left top would leave a leftover shift every time.
		if (p != null && p.length > 0 && scoreboard != null) {
			float[] anchored = {p[0] + scoreboard[2] * p[2], p[1] + scoreboard[3] * p[2] / 2, p[2]};
			scoreboardMoved = move(anchored, new int[] {scoreboard[0] + scoreboard[2], scoreboard[1] + scoreboard[3] / 2});
		} else {
			scoreboardMoved = false;
		}
		if (p != null && p.length == 0) {
			capturing = false;
			return false;
		}
		return true;
	}

	public static void scoreboardEnd() {
		capturing = false;
		if (capX1 > capX0 && capY1 > capY0) {
			scoreboard = new int[] {capX0, capY0, capX1 - capX0, capY1 - capY0};
		}
		if (scoreboardMoved) {
			GlStateManager.popMatrix();
			scoreboardMoved = false;
		}
	}

	/** A background fill the game drew (while drawing the scoreboard, it's the scoreboard's). */
	public static void filled(int x0, int y0, int x1, int y1) {
		// Drawing the scoreboard takes well under 5 ms: a capture still open after that never got its end
		// (another mod cancelled the draw), and later fills belong to other parts.
		if (!capturing || System.nanoTime() - capStarted > CAPTURE_NANOS) {
			capturing = false;
			return;
		}
		capX0 = Math.min(capX0, Math.min(x0, x1));
		capY0 = Math.min(capY0, Math.min(y0, y1));
		capX1 = Math.max(capX1, Math.max(x0, x1));
		capY1 = Math.max(capY1, Math.max(y0, y1));
	}

	/** Before the boss bars are drawn ({@code bars} of them, {@code step} apart); false: don't draw them. */
	public static boolean bossBegin(int bars, int step) {
		if (bars <= 0) {
			bossMoved = false;
			return true;
		}
		Window window = new Window(MinecraftClient.getInstance());
		int w = (int) window.getScaledWidth();
		int h = (int) window.getScaledHeight();
		int shown = Math.max(1, Math.min(bars, (h / 3 - 12) / step + 1));
		int[] natural = {w / 2 - VanillaParts.BOSSBAR_WIDTH / 2, 2, VanillaParts.BOSSBAR_WIDTH, shown * step - 4};
		float[] p = ArcticClient.hud().vanillaPlacement(VanillaParts.BOSSBAR, natural[0], natural[1], natural[2], natural[3]);
		bossMoved = move(p, natural);
		return p == null || p.length > 0;
	}

	public static void bossEnd() {
		if (bossMoved) {
			GlStateManager.popMatrix();
			bossMoved = false;
		}
	}

	private static boolean move(float[] p, int[] natural) {
		if (p == null || p.length == 0 || natural == null) {
			return false;
		}
		GlStateManager.pushMatrix();
		GlStateManager.translate(p[0], p[1], 0f);
		GlStateManager.scale(p[2], p[2], 1f);
		GlStateManager.translate((float) -natural[0], (float) -natural[1], 0f);
		return true;
	}
}
