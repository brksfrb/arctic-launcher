package com.arcticlauncher.mod;

import java.util.ArrayList;
import java.util.List;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.gui.screens.TitleScreen;
import org.spongepowered.asm.mixin.extensibility.IMixinInfo;
import org.spongepowered.asm.mixin.transformer.ClassInfo;

/**
 * Whether Minecraft's own title screen stays instead of Arctic's: other mods put buttons and menus
 * there (Essential's account and wardrobe bar, Mod Menu's Mods button, account switchers), which
 * Arctic's page wouldn't have. It stays, in the Arctic style and with the Arctic button, when a mod
 * other than Arctic changes it. Decided once.
 */
public final class VanillaTitle {
	/** Mods whose title screen menu isn't a mixin of their own (drawn over it, or added through a loader of theirs). */
	private static final String[] KNOWN = {"essential", "modmenu"};
	/** Mixin configs that change the title screen without adding to it. */
	private static final String[] IGNORED = {"arctic", "fabric-"};

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

	/** Mods (ids) and mixin configs that change the title screen, besides Arctic's own. */
	private static List<String> changedBy() {
		List<String> by = new ArrayList<>();
		for (String id : KNOWN) {
			if (FabricLoader.getInstance().isModLoaded(id)) {
				by.add(id);
			}
		}
		try {
			// Mixins.getConfigs() is the pending queue: configs are removed as they're selected,
			// before the first title screen. Read the mixins actually applied to the loaded class.
			ClassInfo title = ClassInfo.fromCache(TitleScreen.class.getName());
			if (title != null) {
				for (IMixinInfo mixin : title.getAppliedMixins()) {
					String name = mixin.getConfig().getName();
					if (!ignored(name) && !by.contains(name)) {
						by.add(name);
					}
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
