package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.render.item.ItemRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.ModifyConstant;

/** Glint Colorizer: the shimmer on enchanted items you hold or see in menus takes the player's color (armor keeps the game's). */
@Mixin(ItemRenderer.class)
abstract class GlintColorLegacyMixin {
	/** The game's purple, 0xFF8040CC, in the two passes of the glint. */
	@ModifyConstant(method = "renderGlint", constant = @Constant(intValue = -8372020))
	private int arctic$glint(int vanilla) {
		ClientConfig c = ArcticClient.config();
		return c != null && c.glintColor != 0 ? 0xFF000000 | c.glintColor : vanilla;
	}
}
