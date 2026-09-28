package com.arcticlauncher.client.looks;

/** A player's published look: texture hashes (null = none) and when we asked. */
public final class Look {
	public final String skin;
	public final boolean slim;
	public final String cape;
	/** Worn 3D cosmetics (catalog ids). */
	public final java.util.List<String> cosmetics;
	/** Playing with Arctic right now (checked in recently). */
	public final boolean arctic;
	final long fetched;

	Look(String skin, boolean slim, String cape, java.util.List<String> cosmetics, boolean arctic, long fetched) {
		this.skin = skin;
		this.slim = slim;
		this.cape = cape;
		this.cosmetics = cosmetics;
		this.arctic = arctic;
		this.fetched = fetched;
	}

	boolean isEmpty() {
		return skin == null && cape == null && cosmetics.isEmpty();
	}
}
