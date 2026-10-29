package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.item.ItemStack;
import net.minecraft.item.PotionItem;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Shiny Pots: every potion (water bottles and the like too, thrown or drunk) carries the enchantment glint. */
@Mixin(PotionItem.class)
abstract class ShinyPotsLegacyMixin {
	@Inject(method = "hasEnchantmentGlint", at = @At("HEAD"), cancellable = true)
	private void arctic$shiny(ItemStack stack, CallbackInfoReturnable<Boolean> cir) {
		if (ArcticClient.config() != null && ArcticClient.config().shinyPots) {
			cir.setReturnValue(true);
		}
	}
}
