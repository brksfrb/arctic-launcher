package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Gfx;

public final class ArmorHider {
	public enum Piece {
		HELMET, CHESTPLATE, LEGGINGS, BOOTS;

		boolean hidden(ClientConfig config) {
			switch (this) {
				case HELMET: return config.hideHelmet;
				case CHESTPLATE: return config.hideChestplate;
				case LEGGINGS: return config.hideLeggings;
				case BOOTS: return config.hideBoots;
				default: throw new AssertionError(this);
			}
		}

		void flip(ClientConfig config) {
			switch (this) {
				case HELMET: config.hideHelmet = !config.hideHelmet; break;
				case CHESTPLATE: config.hideChestplate = !config.hideChestplate; break;
				case LEGGINGS: config.hideLeggings = !config.hideLeggings; break;
				case BOOTS: config.hideBoots = !config.hideBoots; break;
			}
		}
	}

	private static final Piece[] PIECES = Piece.values();
	private static Object player;
	private static int previousHurtTime;
	private static long lastHit;
	private static boolean sawHit;

	private ArmorHider() {}

	public static void tick(Object localPlayer, int hurtTime) {
		if (localPlayer != player) {
			player = localPlayer;
			previousHurtTime = 0;
			sawHit = false;
		}
		if (hurtTime > previousHurtTime) {
			lastHit = System.nanoTime();
			sawHit = true;
		}
		previousHurtTime = hurtTime;
	}

	public static boolean hides(Piece piece) {
		ClientConfig config = ArcticClient.config();
		if (config == null || !config.armorHider || piece == null || !piece.hidden(config)) {
			return false;
		}
		return !config.showArmorWhenHit || !sawHit
				|| System.nanoTime() - lastHit >= config.showArmorSeconds * 1_000_000_000.0;
	}

	public static void drawButtons(Gfx gfx, int left, int top, int mouseX, int mouseY) {
		ClientConfig config = ArcticClient.config();
		if (config == null || !config.armorHider) return;
		int x = left + 1;
		for (int i = 0; i < PIECES.length; i++) {
			int y = top + 11 + 18 * i;
			boolean hovered = inside(mouseX, mouseY, x, y);
			boolean hidden = PIECES[i].hidden(config);
			gfx.fill(x, y, x + 6, y + 10, hovered ? 0xFFB9E9FF : 0xFF454D56);
			gfx.fill(x + 1, y + 1, x + 5, y + 9, hidden ? 0xFF286887 : 0xFF252B30);
			int markerY = hidden ? y + 2 : y + 6;
			gfx.fill(x + 2, markerY, x + 4, markerY + 2, hidden ? 0xFF8DDCFF : 0xFFB4BFC8);
		}
	}

	public static boolean click(int left, int top, double mouseX, double mouseY, int button) {
		ClientConfig config = ArcticClient.config();
		if (button != 0 || config == null || !config.armorHider) return false;
		for (int i = 0; i < PIECES.length; i++) {
			if (inside(mouseX, mouseY, left + 1, top + 11 + 18 * i)) {
				PIECES[i].flip(config);
				ArcticClient.saveConfig();
				return true;
			}
		}
		return false;
	}

	private static boolean inside(double mouseX, double mouseY, int x, int y) {
		return mouseX >= x && mouseX < x + 6 && mouseY >= y && mouseY < y + 10;
	}
}
