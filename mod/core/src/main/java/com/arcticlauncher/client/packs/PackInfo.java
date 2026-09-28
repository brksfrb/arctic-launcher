package com.arcticlauncher.client.packs;

import java.io.File;

/** A resource pack the game knows about. */
public final class PackInfo {
	public final String id;
	public final String title;
	public final String description;
	public final boolean enabled;
	/** Can't be turned off (Minecraft's own). */
	public final boolean required;
	/** Its zip or folder in the resourcepacks folder, or null (built in). */
	public final File file;

	public PackInfo(String id, String title, String description, boolean enabled, boolean required, File file) {
		this.id = id;
		this.title = title;
		this.description = description == null ? "" : description;
		this.enabled = enabled;
		this.required = required;
		this.file = file;
	}
}
