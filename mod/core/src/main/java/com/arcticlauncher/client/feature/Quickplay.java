package com.arcticlauncher.client.feature;

/**
 * Quickplay: pick a minigame and join it with one click instead of typing its
 * command. The list is Hypixel's /play ids; other servers just ignore what
 * they don't know.
 */
public final class Quickplay {
	/** Group → {label, command} pairs. */
	public static final String[] GROUPS = {"Lobby", "Bed Wars", "SkyWars", "Duels", "Murder Mystery", "TNT Games", "Build Battle",
			"Blitz SG", "Mega Walls", "Arcade", "SkyBlock"};
	private static final String[][][] GAMES = {
			{{"Main lobby", "/lobby"}, {"Limbo", "/limbo"}},
			{{"Solo", "/play bedwars_eight_one"}, {"Doubles", "/play bedwars_eight_two"}, {"3v3v3v3", "/play bedwars_four_three"},
					{"4v4v4v4", "/play bedwars_four_four"}, {"4v4", "/play bedwars_two_four"}},
			{{"Solo Normal", "/play solo_normal"}, {"Solo Insane", "/play solo_insane"}, {"Doubles Normal", "/play teams_normal"},
					{"Doubles Insane", "/play teams_insane"}},
			{{"Classic", "/play duels_classic_duel"}, {"Sumo", "/play duels_sumo_duel"}, {"Bridge", "/play duels_bridge_duel"},
					{"UHC", "/play duels_uhc_duel"}, {"OP", "/play duels_op_duel"}, {"Combo", "/play duels_combo_duel"},
					{"Bow", "/play duels_bow_duel"}, {"Blitz", "/play duels_blitz_duel"}, {"SkyWars", "/play duels_sw_duel"}},
			{{"Classic", "/play murder_classic"}, {"Assassins", "/play murder_assassins"}, {"Infection", "/play murder_infection"},
					{"Double Up", "/play murder_double_up"}},
			{{"TNT Run", "/play tnt_tntrun"}, {"PVP Run", "/play tnt_pvprun"}, {"Bow Spleef", "/play tnt_bowspleef"},
					{"TNT Tag", "/play tnt_tntag"}, {"Wizards", "/play tnt_capture"}},
			{{"Solo", "/play build_battle_solo_normal"}, {"Teams", "/play build_battle_teams_normal"}},
			{{"Solo", "/play blitz_solo_normal"}, {"Teams", "/play blitz_teams_normal"}},
			{{"Standard", "/play mw_standard"}, {"Face Off", "/play mw_face_off"}},
			{{"Party Games", "/play arcade_party_games_1"}, {"Hole in the Wall", "/play arcade_hole_in_the_wall"},
					{"Pixel Painters", "/play arcade_pixel_painters"}},
			{{"SkyBlock", "/play skyblock"}},
	};

	private Quickplay() {}

	public static String[] games(int group) {
		String[][] list = GAMES[Math.floorMod(group, GAMES.length)];
		String[] names = new String[list.length];
		for (int i = 0; i < list.length; i++) {
			names[i] = list[i][0];
		}
		return names;
	}

	public static String command(int group, int game) {
		String[][] list = GAMES[Math.floorMod(group, GAMES.length)];
		return list[Math.floorMod(game, list.length)][1];
	}
}
