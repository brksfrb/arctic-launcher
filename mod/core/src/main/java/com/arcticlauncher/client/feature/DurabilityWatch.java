package com.arcticlauncher.client.feature;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.notice.Notices;

import com.arcticlauncher.client.Lang;
/** A pop-up when worn armor or the held tool is about to break (once per item until repaired). */
public final class DurabilityWatch {
	/** The threshold a config without one (or with a nonsense one) gets, in percent. */
	public static final int DEFAULT_PERCENT = 10;
	public static final int MIN_PERCENT = 1;
	public static final int MAX_PERCENT = 50;
	/** {@link Platform#durability()} tiers the "diamond / netherite only" filter keeps. */
	public static final String DIAMOND = "diamond";
	public static final String NETHERITE = "netherite";
	/** The row's slot name for the item in the main hand. */
	public static final String MAIN_HAND = "mainhand";
	/** Checked this often (ticks). */
	private static final int EVERY = 20;

	private final Set<String> warned = new HashSet<String>();
	private int ticks;

	/** Whether {@code left} of {@code max} is at or below {@code percent} (whole numbers, no rounding). */
	public static boolean low(int left, int max, int percent) {
		return max > 0 && (long) left * 100 <= (long) max * clampPercent(percent);
	}

	public static int clampPercent(int percent) {
		return Math.max(MIN_PERCENT, Math.min(MAX_PERCENT, percent));
	}

	/** Whether an item of this tier is watched: everything, or with the filter on only diamond (and netherite where the version has it). */
	public static boolean watched(String tier, boolean diamondOnly) {
		return !diamondOnly || DIAMOND.equals(tier) || NETHERITE.equals(tier);
	}

	/** The tier of the vanilla item with this registry path ("diamond_pickaxe", "netherite_chestplate"...): "diamond", "netherite" or "other". */
	public static String tierOf(String path) {
		if (path != null && path.startsWith("diamond_")) {
			return DIAMOND;
		}
		return path != null && path.startsWith("netherite_") ? NETHERITE : "other";
	}

	/** Whether a row of {@link Platform#durability()} is low under this config. */
	static boolean lowRow(Object[] item, ClientConfig c) {
		String tier = item.length > 4 ? (String) item[4] : null;
		return watched(tier, c.durabilityDiamondOnly) && low((Integer) item[1], (Integer) item[2], c.durabilityPercent);
	}

	/**
	 * Whether using (mining, attacking, interacting with) the item in hand is blocked: the switch is on
	 * and that item is damageable, watched and at or below the threshold. Armor is never blocked.
	 */
	public static boolean useBlocked(ClientConfig c, Platform platform) {
		return c.durabilityWarning && c.durabilityBlockUse && platform != null && useBlocked(c, platform.durability());
	}

	/** {@link #useBlocked(ClientConfig, Platform)} for rows already read from {@link Platform#durability()}. */
	static boolean useBlocked(ClientConfig c, List<Object[]> rows) {
		if (!c.durabilityWarning || !c.durabilityBlockUse) {
			return false;
		}
		for (Object[] item : rows) {
			if (MAIN_HAND.equals(item[3]) && lowRow(item, c)) {
				return true;
			}
		}
		return false;
	}

	/** {@link #useBlocked} for the running client; for the game's input hooks. */
	public static boolean useBlocked() {
		return useBlocked(ArcticClient.config(), ArcticClient.platform());
	}

	public void tick(Platform platform) {
		tick(platform, ArcticClient.config());
	}

	public void tick(Platform platform, ClientConfig c) {
		if (++ticks % EVERY != 0) {
			return;
		}
		for (Object[] item : scan(platform.durability(), c)) {
			Notices.post(Lang.t("hud.is_about_to_break", item[0]), Lang.t("hud.of_durability_left", item[1], item[2]));
		}
	}

	/** The rows that just got low under {@code c}: each item shows up once, until it is repaired or swapped out. */
	List<Object[]> scan(List<Object[]> items, ClientConfig c) {
		Set<String> low = new HashSet<String>();
		List<Object[]> fresh = new ArrayList<Object[]>();
		for (Object[] item : items) {
			if (lowRow(item, c)) {
				String key = item[3] + ":" + item[0];
				low.add(key);
				if (warned.add(key)) {
					fresh.add(item);
				}
			}
		}
		// Repaired or swapped: warn again next time it gets low.
		warned.retainAll(low);
		return fresh;
	}
}
