package com.arcticlauncher.legacy;

import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.ingame.SurvivalInventoryScreen;

/** Your player in a box, looking toward the mouse (the Looks tab preview). */
final class LegacyPlayerPreview {
	private LegacyPlayerPreview() {}

	static void draw(int x0, int y0, int x1, int y1, int scale, int mouseX, int mouseY) {
		MinecraftClient mc = MinecraftClient.getInstance();
		if (mc.player == null) {
			return;
		}
		int cx = (x0 + x1) / 2;
		int feet = y1 - (y1 - y0) / 12;
		GlStateManager.color(1f, 1f, 1f, 1f);
		SurvivalInventoryScreen.renderEntity(cx, feet, scale, cx - mouseX, feet - scale * 2 - mouseY, mc.player);
	}
}
