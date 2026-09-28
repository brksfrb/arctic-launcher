package com.arcticlauncher.client.replay;

import java.util.ArrayList;
import java.util.List;

/**
 * {@code metaData.json} inside a replay, in ReplayMod's format (so its
 * files and ours open in either), plus Arctic's own {@code arctic.json}.
 */
public final class ReplayMeta {
	public boolean singleplayer;
	public String serverName;
	/** Length in milliseconds. */
	public int duration;
	/** When it was recorded (ms since 1970). */
	public long date;
	public String fileFormat = "MCPR";
	public int fileFormatVersion = 14;
	public int protocol;
	public String generator = "Arctic Client";
	/** The recording player's entity id (in ReplayMod's files: -1 when unknown). */
	public int selfId = -1;
	public String[] players = new String[0];
	public String mcversion;

	/** Arctic's extras ({@code arctic.json}). */
	public static final class Extras {
		public int version = 1;
		public String selfName;
		public String selfUuid;
		/** Moments marked while playing (ms), each the end of a 2D clip. */
		public List<Integer> moments = new ArrayList<Integer>();
		/** How far back a moment reaches (seconds). */
		public int clipSeconds = 30;
		/** Camera path keyframes. */
		public List<Keyframe> keyframes = new ArrayList<Keyframe>();

		void fillDefaults() {
			if (moments == null) {
				moments = new ArrayList<Integer>();
			}
			if (keyframes == null) {
				keyframes = new ArrayList<Keyframe>();
			}
			if (clipSeconds <= 0) {
				clipSeconds = 30;
			}
		}
	}
}
