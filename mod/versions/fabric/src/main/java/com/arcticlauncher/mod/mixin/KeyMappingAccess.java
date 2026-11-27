package com.arcticlauncher.mod.mixin;

//#if MC < 1.21.9
import java.util.Map;

import net.minecraft.client.KeyMapping;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * The category order Controls sorts key mappings by. Minecraft's constructor
 * does not add a new category to it, and KeyMapping.compareTo throws on a
 * category it does not know, so a mapping with its own category must be added
 * here first. From 1.21.9 categories are registered objects instead.
 */
@Mixin(KeyMapping.class)
public interface KeyMappingAccess {
	@Accessor("CATEGORY_SORT_ORDER")
	static Map<String, Integer> arctic$categoryOrder() {
		throw new AssertionError();
	}
}
//#endif
