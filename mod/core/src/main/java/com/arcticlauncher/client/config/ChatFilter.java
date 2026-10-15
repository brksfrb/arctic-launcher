package com.arcticlauncher.client.config;

/**
 * A saved chat filter: a name and words. A line matches when it has any of the words (case
 * ignored); a word with a leading "-" keeps matching lines out.
 */
public final class ChatFilter {
	public String name = "";
	/** Words separated by commas: "diamond, trade, -spam". */
	public String words = "";

	public ChatFilter() {}

	public ChatFilter(String name, String words) {
		this.name = name;
		this.words = words;
	}
}
