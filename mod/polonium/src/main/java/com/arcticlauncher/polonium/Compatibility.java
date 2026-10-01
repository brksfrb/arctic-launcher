package com.arcticlauncher.polonium;

import java.util.Set;
import java.util.TreeSet;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.spongepowered.asm.mixin.extensibility.IMixinInfo;
import org.spongepowered.asm.mixin.transformer.ClassInfo;

/**
 * Polonium takes over the upload loop rather than patching inside it, so if
 * another mod also changes {@code StagedVertexBuffer}, Polonium steps aside
 * rather than risk skipping that mod's work. Mods known to be fine are listed.
 */
final class Compatibility {
	private static final Logger LOG = LoggerFactory.getLogger("Polonium");
	private static final String TARGET = "net/minecraft/client/renderer/StagedVertexBuffer";
	/** Other mods' changes that keep working with Polonium (it calls the code they change). */
	private static final Set<String> KNOWN = Set.of(
			// Sodium: picks each see-through face's sort point, in decodeSortingPoints, which Polonium calls.
			"net.caffeinemc.mods.sodium.mixin.features.render.immediate.buffer_builder.sorting.StagedVertexBufferMixin");
	private static volatile Boolean uploadIsOurs;

	private Compatibility() {}

	static boolean uploadIsOurs() {
		Boolean known = uploadIsOurs;
		if (known == null) {
			known = check();
			uploadIsOurs = known;
		}
		return known;
	}

	private static boolean check() {
		ClassInfo info = ClassInfo.forName(TARGET);
		if (info == null) {
			return true;
		}
		Set<String> others = new TreeSet<>();
		for (IMixinInfo mixin : info.getAppliedMixins()) {
			String name = mixin.getClassName();
			if (!name.startsWith("com.arcticlauncher.polonium.") && !KNOWN.contains(name)) {
				others.add(name);
			}
		}
		if (others.isEmpty()) {
			LOG.info("Polonium: parallel entity upload on ({} threads)", Runtime.getRuntime().availableProcessors());
			return true;
		}
		LOG.warn("Polonium: other mods change entity uploads ({}); leaving them to it", others);
		return false;
	}
}
