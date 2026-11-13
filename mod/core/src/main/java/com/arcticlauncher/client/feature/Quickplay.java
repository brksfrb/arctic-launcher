package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.Lang;

/**
 * Quickplay: pick a minigame and join it with one click instead of typing its
 * command. The list is Hypixel's /play ids; other servers just ignore what
 * they don't know.
 */
public final class Quickplay {
	/** Group → {label, command} pairs. */
	public static final String[] GROUPS = {"mods.quickplay.lobby", "mods.quickplay.bed_wars", "mods.quickplay.skywars", "mods.quickplay.duels", "mods.quickplay.murder_mystery", "mods.quickplay.tnt_games", "mods.quickplay.build_battle",
			"mods.quickplay.blitz_sg", "mods.quickplay.mega_walls", "mods.quickplay.arcade", "mods.quickplay.skyblock"};
	private static final String[][][] GAMES = {
			{{"mods.quickplay.main_lobby", "/lobby"}, {"mods.quickplay.limbo", "/limbo"}},
			{{"mods.quickplay.solo", "/play bedwars_eight_one"}, {"mods.quickplay.doubles", "/play bedwars_eight_two"}, {"mods.quickplay.3v3v3v3", "/play bedwars_four_three"},
					{"mods.quickplay.4v4v4v4", "/play bedwars_four_four"}, {"mods.quickplay.4v4", "/play bedwars_two_four"}},
			{{"mods.quickplay.solo_normal", "/play solo_normal"}, {"mods.quickplay.solo_insane", "/play solo_insane"}, {"mods.quickplay.doubles_normal", "/play teams_normal"},
					{"mods.quickplay.doubles_insane", "/play teams_insane"}},
			{{"mods.quickplay.classic", "/play duels_classic_duel"}, {"mods.quickplay.sumo", "/play duels_sumo_duel"}, {"mods.quickplay.bridge", "/play duels_bridge_duel"},
					{"mods.quickplay.uhc", "/play duels_uhc_duel"}, {"mods.quickplay.op", "/play duels_op_duel"}, {"mods.quickplay.combo", "/play duels_combo_duel"},
					{"mods.quickplay.bow", "/play duels_bow_duel"}, {"mods.quickplay.blitz", "/play duels_blitz_duel"}, {"mods.quickplay.skywars", "/play duels_sw_duel"}},
			{{"mods.quickplay.classic", "/play murder_classic"}, {"mods.quickplay.assassins", "/play murder_assassins"}, {"mods.quickplay.infection", "/play murder_infection"},
					{"mods.quickplay.double_up", "/play murder_double_up"}},
			{{"mods.quickplay.tnt_run", "/play tnt_tntrun"}, {"mods.quickplay.pvp_run", "/play tnt_pvprun"}, {"mods.quickplay.bow_spleef", "/play tnt_bowspleef"},
					{"mods.quickplay.tnt_tag", "/play tnt_tntag"}, {"mods.quickplay.wizards", "/play tnt_capture"}},
			{{"mods.quickplay.solo", "/play build_battle_solo_normal"}, {"mods.quickplay.teams", "/play build_battle_teams_normal"}},
			{{"mods.quickplay.solo", "/play blitz_solo_normal"}, {"mods.quickplay.teams", "/play blitz_teams_normal"}},
			{{"mods.quickplay.standard", "/play mw_standard"}, {"mods.quickplay.face_off", "/play mw_face_off"}},
			{{"mods.quickplay.party_games", "/play arcade_party_games_1"}, {"mods.quickplay.hole_in_the_wall", "/play arcade_hole_in_the_wall"},
					{"mods.quickplay.pixel_painters", "/play arcade_pixel_painters"}},
			{{"mods.quickplay.skyblock", "/play skyblock"}},
	};

	private Quickplay() {}

	public static String[] games(int group) {
		String[][] list = GAMES[Math.floorMod(group, GAMES.length)];
		String[] names = new String[list.length];
		for (int i = 0; i < list.length; i++) {
			names[i] = Lang.t(list[i][0]);
		}
		return names;
	}

	public static String command(int group, int game) {
		String[][] list = GAMES[Math.floorMod(group, GAMES.length)];
		return list[Math.floorMod(game, list.length)][1];
	}
}
