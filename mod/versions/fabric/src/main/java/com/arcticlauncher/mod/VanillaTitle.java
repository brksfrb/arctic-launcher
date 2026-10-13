package com.arcticlauncher.mod;

import java.util.ArrayList;
import java.util.List;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.gui.screens.TitleScreen;
import org.spongepowered.asm.mixin.transformer.meta.MixinMerged;

/**
 * Whether Minecraft's own title screen stays instead of Arctic's: other mods put buttons and menus
 * there (Essential's account and wardrobe bar, Mod Menu's Mods button, account switchers), which
 * Arctic's page wouldn't have. It stays, in the Arctic style and with the Arctic button, when a mod
 * other than Arctic changes it. Decided once.
 */
public final class VanillaTitle {
	/** Mods whose title screen menu isn't a mixin of their own (drawn over it, or added through a loader of theirs). */
	private static final String[] KNOWN = {"essential", "modmenu"};
	/** Mixin owners that change the title screen without adding their own menus. */
	private static final String[] IGNORED = {"com.arcticlauncher.", "net.fabricmc.fabric."};

	private static Boolean keep;

	private VanillaTitle() {}

	public static boolean keep() {
		if (keep == null) {
			List<String> by = changedBy();
			keep = !by.isEmpty();
			if (keep) {
				ArcticMod.LOG.info("Minecraft's title screen stays (other mods add to it: {})", String.join(", ", by));
			}
		}
		return keep;
	}

	/** Mods (ids) and applied mixins that change the title screen, besides Arctic's own. */
	private static List<String> changedBy() {
		List<String> by = new ArrayList<>();
		for (String id : KNOWN) {
			if (FabricLoader.getInstance().isModLoaded(id)) {
				by.add(id);
			}
		}
		try {
			// Read the merged methods on the actual runtime class. The pending config queue
			// is consumed at startup and Mixin's ClassInfo cache is not reliable from game code.
			for (java.lang.reflect.Method method : TitleScreen.class.getDeclaredMethods()) {
				MixinMerged merged = method.getAnnotation(MixinMerged.class);
				if (merged != null && !ignored(merged.mixin()) && !by.contains(merged.mixin())) {
					by.add(merged.mixin());
				}
			}
		} catch (RuntimeException | LinkageError e) {
			// If a loader changes this API, keep the screen so third-party controls aren't lost.
			ArcticMod.LOG.warn("Could not inspect title screen mixins; keeping Minecraft's title screen", e);
			by.add("mixin metadata unavailable");
		}
		return by;
	}

	private static boolean ignored(String config) {
		for (String prefix : IGNORED) {
			if (config.startsWith(prefix)) {
				return true;
			}
		}
		return false;
	}
}
