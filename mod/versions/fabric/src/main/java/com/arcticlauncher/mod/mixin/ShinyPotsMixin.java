package com.arcticlauncher.mod.mixin;

//#if MC >= 1.13
import com.arcticlauncher.client.ArcticClient;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.PotionItem;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Shiny Pots: every potion (water bottles and the like too, thrown or drunk) carries the enchantment glint. */
@Mixin(ItemStack.class)
abstract class ShinyPotsMixin {
	@Inject(method = "hasFoil", at = @At("RETURN"), cancellable = true)
	private void arctic$shiny(CallbackInfoReturnable<Boolean> cir) {
		if (!cir.getReturnValueZ() && ArcticClient.config() != null && ArcticClient.config().shinyPots
				&& ((ItemStack) (Object) this).getItem() instanceof PotionItem) {
			cir.setReturnValue(true);
		}
	}
}
//#endif
