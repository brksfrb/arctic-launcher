package com.arcticlauncher.mod.mixin;

import net.minecraft.client.ClientBrandRetriever;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The brand the game tells servers it is. A Vanilla instance runs on Fabric underneath, which
 * says "fabric"; the launcher asks for "vanilla" there, so servers' anti-cheat doesn't take
 * a plain Vanilla player for a modded client.
 */
@Mixin(ClientBrandRetriever.class)
abstract class ClientBrandMixin {
	@Inject(method = "getClientModName", at = @At("HEAD"), cancellable = true, remap = false)
	private static void arctic$brand(CallbackInfoReturnable<String> cir) {
		String brand = System.getProperty("arctic.brand");
		if (brand != null && !brand.isEmpty()) {
			cir.setReturnValue(brand);
		}
	}
}
