package net.fabricmc.loader.api;

import net.fabricmc.loader.api.metadata.ModMetadata;

/** Stand-in for Fabric's ModContainer: only the metadata. */
public interface ModContainer {
	ModMetadata getMetadata();
}
