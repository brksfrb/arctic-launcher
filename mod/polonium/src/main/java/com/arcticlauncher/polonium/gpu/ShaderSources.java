package com.arcticlauncher.polonium.gpu;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import net.minecraft.resources.Identifier;
import org.jspecify.annotations.Nullable;

/** Polonium's shaders, served from its jar (so no resource pack or Fabric API is needed). */
public final class ShaderSources {
	private static final java.util.Map<Identifier, String> FILES = java.util.Map.of(
			ModelMesh.SHADER, "/polonium/entity_instanced.vsh",
			ItemMesh.SHADER, "/polonium/item_instanced.vsh");
	private static final java.util.Map<Identifier, String> LOADED = new java.util.concurrent.ConcurrentHashMap<>();

	private ShaderSources() {}

	/** The source of one of Polonium's vertex shaders, or null if {@code id} isn't one. */
	public static @Nullable String vertex(Identifier id) {
		String file = FILES.get(id);
		if (file == null) {
			return null;
		}
		return LOADED.computeIfAbsent(id, key -> read(file));
	}

	private static String read(String file) {
		try (InputStream in = ShaderSources.class.getResourceAsStream(file)) {
			if (in == null) {
				throw new IllegalStateException(file + " is missing from the jar");
			}
			return new String(in.readAllBytes(), StandardCharsets.UTF_8);
		} catch (IOException e) {
			throw new UncheckedIOException(e);
		}
	}
}
