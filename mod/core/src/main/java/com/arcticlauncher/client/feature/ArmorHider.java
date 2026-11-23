package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Gfx;

public final class ArmorHider {
	public enum Piece {
		HELMET("mods.hide_helmet"), CHESTPLATE("mods.hide_chestplate"),
		LEGGINGS("mods.hide_leggings"), BOOTS("mods.hide_boots");

		private final String label;

		Piece(String label) {
			this.label = label;
		}

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
	private static final int SLOT_X = 8;
	private static final int FIRST_SLOT_Y = 8;
	private static final int SLOT_STEP = 18;
	private static final int BOX_SIZE = 6;
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

	public static void drawCheckboxes(Gfx gfx, int left, int top, int mouseX, int mouseY) {
		ClientConfig config = ArcticClient.config();
		if (config == null || !config.armorHider) return;
		gfx.newLayer();
		int x = left + SLOT_X;
		Piece hovered = hoveredPiece(left, top, mouseX, mouseY);
		for (int i = 0; i < PIECES.length; i++) {
			int y = top + FIRST_SLOT_Y + SLOT_STEP * i;
			gfx.fill(x, y, x + BOX_SIZE, y + BOX_SIZE, 0xFF373737);
			gfx.fill(x + 1, y + 1, x + 5, y + 5, PIECES[i] == hovered ? 0xFFFFFFFF : 0xFFC6C6C6);
			if (PIECES[i].hidden(config)) {
				int ink = 0xFF252525;
				gfx.fill(x + 1, y + 3, x + 2, y + 4, ink);
				gfx.fill(x + 2, y + 4, x + 3, y + 5, ink);
				gfx.fill(x + 3, y + 3, x + 4, y + 4, ink);
				gfx.fill(x + 4, y + 2, x + 5, y + 3, ink);
			}
		}
	}

	public static boolean drawTooltip(Gfx gfx, int left, int top, int mouseX, int mouseY) {
		Piece piece = hoveredPiece(left, top, mouseX, mouseY);
		if (piece == null) return false;
		String label = Lang.t(piece.label);
		int w = gfx.textWidth(label) + 8;
		int x = Math.max(0, Math.min(mouseX + 8, gfx.width() - w));
		int y = Math.max(0, Math.min(mouseY + 8, gfx.height() - 16));
		gfx.newLayer();
		gfx.fill(x, y, x + w, y + 16, 0xFF504060);
		gfx.fill(x + 1, y + 1, x + w - 1, y + 15, 0xF0100010);
		gfx.text(label, x + 4, y + 4, 0xFFFFFFFF, true);
		return true;
	}

	public static boolean click(int left, int top, double mouseX, double mouseY, int button) {
		if (button != 0) return false;
		Piece piece = hoveredPiece(left, top, mouseX, mouseY);
		if (piece == null) return false;
		piece.flip(ArcticClient.config());
		ArcticClient.saveConfig();
		return true;
	}

	public static Piece hoveredPiece(int left, int top, double mouseX, double mouseY) {
		ClientConfig config = ArcticClient.config();
		int x = left + SLOT_X;
		if (config == null || !config.armorHider || mouseX < x || mouseX >= x + BOX_SIZE) return null;
		for (int i = 0; i < PIECES.length; i++) {
			int y = top + FIRST_SLOT_Y + SLOT_STEP * i;
			if (mouseY >= y && mouseY < y + BOX_SIZE) return PIECES[i];
		}
		return null;
	}
}
