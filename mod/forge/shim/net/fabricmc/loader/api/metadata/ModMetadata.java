package net.fabricmc.loader.api.metadata;

import net.fabricmc.loader.api.Version;

/** Stand-in for Fabric's ModMetadata: only the version. */
public interface ModMetadata {
	Version getVersion();
}
