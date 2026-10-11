package com.arcticlauncher.client.tiers;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;

/**
 * A player's tiers on one tier list, as its profile API answers
 * ({@code {"rankings": {"sword": {"tier": 3, "pos": 0, "retired": false, ...}}}}).
 */
public final class TierProfile {
	/** One game mode's tier: tier 1 is the best, high ({@code pos 0}) beats low. */
	public static final class Rank {
		public final String mode;
		public final int tier;
		public final boolean high;
		public final boolean retired;

		Rank(String mode, int tier, boolean high, boolean retired) {
			this.mode = mode;
			this.tier = tier;
			this.high = high;
			this.retired = retired;
		}

		/** "HT3", "LT1"; retired ones start with R. */
		public String label() {
			return (retired ? "R" : "") + (high ? "HT" : "LT") + tier;
		}

		/** The tier lists' own points: HT1 60, LT1 45, HT2 30 ... LT5 1. */
		public int points() {
			int i = (tier - 1) * 2 + (high ? 0 : 1);
			return i >= 0 && i < POINTS.length ? POINTS[i] : 0;
		}
	}

	private static final int[] POINTS = {60, 45, 30, 20, 10, 6, 4, 3, 2, 1};

	public final List<Rank> ranks;

	private TierProfile(List<Rank> ranks) {
		this.ranks = Collections.unmodifiableList(ranks);
	}

	/** Null when it isn't a profile (an error, or a player who was never tested). */
	@SuppressWarnings("deprecation")
	public static TierProfile parse(String json) {
		JsonElement root;
		try {
			// Gson 2.2 (1.8.9) has no JsonParser.parseString.
			root = new JsonParser().parse(json);
		} catch (RuntimeException e) {
			return null;
		}
		if (!root.isJsonObject()) {
			return null;
		}
		JsonElement rankings = root.getAsJsonObject().get("rankings");
		if (rankings == null || !rankings.isJsonObject()) {
			return null;
		}
		List<Rank> out = new ArrayList<Rank>();
		for (Map.Entry<String, JsonElement> e : rankings.getAsJsonObject().entrySet()) {
			Rank r = rank(e.getKey(), e.getValue());
			if (r != null) {
				out.add(r);
			}
		}
		return new TierProfile(out);
	}

	private static Rank rank(String mode, JsonElement value) {
		if (!value.isJsonObject()) {
			return null;
		}
		JsonObject o = value.getAsJsonObject();
		int tier = number(o, "tier", 0);
		if (tier < 1 || tier > 5) {
			return null;
		}
		JsonElement retired = o.get("retired");
		boolean isRetired = retired != null && retired.isJsonPrimitive() && retired.getAsBoolean();
		return new Rank(mode, tier, number(o, "pos", 1) == 0, isRetired);
	}

	private static int number(JsonObject o, String key, int otherwise) {
		JsonElement v = o.get(key);
		if (v == null || !v.isJsonPrimitive()) {
			return otherwise;
		}
		try {
			return v.getAsInt();
		} catch (RuntimeException e) {
			return otherwise;
		}
	}

	/** The tier in this mode, or null. */
	public Rank in(String mode) {
		for (Rank r : ranks) {
			if (r.mode.equals(mode)) {
				return r;
			}
		}
		return null;
	}

	/** Their best tier: a current one over a retired one, then the most points. */
	public Rank best() {
		Rank best = null;
		for (Rank r : ranks) {
			if (best == null || better(r, best)) {
				best = r;
			}
		}
		return best;
	}

	private static boolean better(Rank a, Rank b) {
		if (a.retired != b.retired) {
			return !a.retired;
		}
		return a.points() > b.points();
	}
}
