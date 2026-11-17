package com.arcticlauncher.client.looks;

/** A player's published look: texture hashes (null = none) and when we asked. */
public final class Look {
	public final String skin;
	public final boolean slim;
	public final String cape;
	/** The wearer's published badge, 0 when absent or unknown. */
	public final int badgeStyle;
	/** Worn 3D cosmetics (catalog ids). */
	public final java.util.List<String> cosmetics;
	/** Playing with Arctic right now (checked in recently). */
	public final boolean arctic;
	final long fetched;
	/** How some worn cosmetics sit, by id. */
	final java.util.Map<String, Fit> fits;

	Look(String skin, boolean slim, String cape, java.util.List<String> cosmetics, boolean arctic, long fetched) {
		this(skin, slim, cape, cosmetics, arctic, fetched, java.util.Collections.<String, Fit>emptyMap());
	}

	Look(String skin, boolean slim, String cape, java.util.List<String> cosmetics, boolean arctic, long fetched,
			java.util.Map<String, Fit> fits) {
		this(skin, slim, cape, cosmetics, arctic, fetched, fits, 0);
	}

	Look(String skin, boolean slim, String cape, java.util.List<String> cosmetics, boolean arctic, long fetched,
			java.util.Map<String, Fit> fits, int badgeStyle) {
		this.badgeStyle = com.arcticlauncher.client.feature.Badge.normalize(badgeStyle);
		this.skin = skin;
		this.slim = slim;
		this.cape = cape;
		this.cosmetics = cosmetics;
		this.arctic = arctic;
		this.fetched = fetched;
		this.fits = fits;
	}

	/** How a worn cosmetic sits (as made, unless its wearer moved it). */
	public Fit fit(String id) {
		Fit f = fits.get(id);
		return f == null ? Fit.NONE : f;
	}

	boolean isEmpty() {
		return skin == null && cape == null && cosmetics.isEmpty();
	}
}
