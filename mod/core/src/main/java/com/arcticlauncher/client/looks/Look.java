package com.arcticlauncher.client.looks;

/** A player's published look: texture hashes (null = none) and when we asked. */
public final class Look {
	public final String skin;
	public final boolean slim;
	public final String cape;
	final long fetched;

	Look(String skin, boolean slim, String cape, long fetched) {
		this.skin = skin;
		this.slim = slim;
		this.cape = cape;
		this.fetched = fetched;
	}

	boolean isEmpty() {
		return skin == null && cape == null;
	}
}
