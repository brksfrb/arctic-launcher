package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.hud.VanillaParts;
import net.minecraft.client.Minecraft;

/**
 * Draws the game's scoreboard and boss bar where the HUD editor put them: the game draws them as
 * usual, inside a move (and scale) of the drawing; or not at all when they're turned off. The
 * scoreboard's own box is learned from its background fills (as drawn the frame before), the
 * boss bar's from how many bars there are.
 */
public final class VanillaMove {
	/** The scoreboard's box as last drawn {x, y, w, h}, or null. */
	private static int[] scoreboard;
	private static boolean capturing;
	private static long capStarted;
	private static final long CAPTURE_NANOS = 5_000_000L;
	private static int capX0;
	private static int capY0;
	private static int capX1;
	private static int capY1;
	/** Whether the call being drawn was moved (so its end undoes it). */
	private static boolean scoreboardMoved;
	private static boolean bossMoved;

	private VanillaMove() {}

	/** Before the scoreboard is drawn; false: don't draw it. */
	public static boolean scoreboardBegin(Object graphics) {
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
			scoreboardMoved = move(graphics, anchored, new int[] {scoreboard[0] + scoreboard[2], scoreboard[1] + scoreboard[3] / 2});
		} else {
			scoreboardMoved = false;
		}
		if (p != null && p.length == 0) {
			capturing = false;
			return false;
		}
		return true;
	}

	public static void scoreboardEnd(Object graphics) {
		capturing = false;
		int w = Minecraft.getInstance().getWindow().getGuiScaledWidth();
		int h = Minecraft.getInstance().getWindow().getGuiScaledHeight();
		// A box wider than half the screen or taller than the screen isn't a scoreboard (fills of other parts
		// got in): keep the last good one.
		if (capX1 > capX0 && capY1 > capY0 && capX1 - capX0 <= w / 2 && capY1 - capY0 <= h) {
			scoreboard = new int[] {capX0, capY0, capX1 - capX0, capY1 - capY0};
		}
		if (scoreboardMoved) {
			pop(graphics);
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

	/** Before the boss bars are drawn ({@code bars} of them); false: don't draw them. */
	public static boolean bossBegin(Object graphics, int bars) {
		if (bars <= 0) {
			bossMoved = false;
			return true;
		}
		int w = Minecraft.getInstance().getWindow().getGuiScaledWidth();
		int h = Minecraft.getInstance().getWindow().getGuiScaledHeight();
		// The game stops adding bars a third of the way down.
		int shown = Math.max(1, Math.min(bars, (h / 3 - 12) / VanillaParts.BOSSBAR_STEP + 1));
		int[] natural = {w / 2 - VanillaParts.BOSSBAR_WIDTH / 2, 3, VanillaParts.BOSSBAR_WIDTH, shown * VanillaParts.BOSSBAR_STEP - 4};
		float[] p = ArcticClient.hud().vanillaPlacement(VanillaParts.BOSSBAR, natural[0], natural[1], natural[2], natural[3]);
		bossMoved = move(graphics, p, natural);
		return p == null || p.length > 0;
	}

	public static void bossEnd(Object graphics) {
		if (bossMoved) {
			pop(graphics);
			bossMoved = false;
		}
	}

	/** Move the drawing from the natural box to the placed one; true if it did. */
	private static boolean move(Object graphics, float[] p, int[] natural) {
		if (p == null || p.length == 0 || natural == null) {
			return false;
		}
		float x = p[0];
		float y = p[1];
		float s = p[2];
		//#if MC >= 26.1
		org.joml.Matrix3x2fStack pose = ((net.minecraft.client.gui.GuiGraphicsExtractor) graphics).pose();
		pose.pushMatrix();
		pose.translate(x, y);
		pose.scale(s, s);
		pose.translate(-natural[0], -natural[1]);
		//#elif MC >= 1.21.6
		org.joml.Matrix3x2fStack pose = ((net.minecraft.client.gui.GuiGraphics) graphics).pose();
		pose.pushMatrix();
		pose.translate(x, y);
		pose.scale(s, s);
		pose.translate(-natural[0], -natural[1]);
		//#elif MC >= 1.20
		com.mojang.blaze3d.vertex.PoseStack pose = ((net.minecraft.client.gui.GuiGraphics) graphics).pose();
		pose.pushPose();
		pose.translate(x, y, 0);
		pose.scale(s, s, 1);
		pose.translate(-natural[0], -natural[1], 0);
		//#elif MC >= 1.16
		com.mojang.blaze3d.vertex.PoseStack pose = (com.mojang.blaze3d.vertex.PoseStack) graphics;
		pose.pushPose();
		pose.translate(x, y, 0);
		pose.scale(s, s, 1);
		pose.translate(-natural[0], -natural[1], 0);
		//#elif MC >= 1.15
		com.mojang.blaze3d.systems.RenderSystem.pushMatrix();
		com.mojang.blaze3d.systems.RenderSystem.translatef(x, y, 0);
		com.mojang.blaze3d.systems.RenderSystem.scalef(s, s, 1);
		com.mojang.blaze3d.systems.RenderSystem.translatef(-natural[0], -natural[1], 0);
		//#else
		com.mojang.blaze3d.platform.GlStateManager.pushMatrix();
		com.mojang.blaze3d.platform.GlStateManager.translatef(x, y, 0);
		com.mojang.blaze3d.platform.GlStateManager.scalef(s, s, 1);
		com.mojang.blaze3d.platform.GlStateManager.translatef(-natural[0], -natural[1], 0);
		//#endif
		return true;
	}

	private static void pop(Object graphics) {
		//#if MC >= 26.1
		((net.minecraft.client.gui.GuiGraphicsExtractor) graphics).pose().popMatrix();
		//#elif MC >= 1.21.6
		((net.minecraft.client.gui.GuiGraphics) graphics).pose().popMatrix();
		//#elif MC >= 1.20
		((net.minecraft.client.gui.GuiGraphics) graphics).pose().popPose();
		//#elif MC >= 1.16
		((com.mojang.blaze3d.vertex.PoseStack) graphics).popPose();
		//#elif MC >= 1.15
		com.mojang.blaze3d.systems.RenderSystem.popMatrix();
		//#else
		com.mojang.blaze3d.platform.GlStateManager.popMatrix();
		//#endif
	}
}
