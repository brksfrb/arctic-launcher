package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.Cosmetics;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.core.ClientAsset;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.player.PlayerSkin;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Swaps in the player's Arctic cape (also used for their elytra). */
@Mixin(AbstractClientPlayer.class)
abstract class AbstractClientPlayerMixin {
	@Inject(method = "getSkin", at = @At("RETURN"), cancellable = true)
	private void arctic$cape(CallbackInfoReturnable<PlayerSkin> cir) {
		Identifier cape = Cosmetics.capeFor(((AbstractClientPlayer) (Object) this).getUUID());
		if (cape == null) {
			return;
		}
		PlayerSkin skin = cir.getReturnValue();
		ClientAsset.ResourceTexture texture = new ClientAsset.ResourceTexture(cape, cape);
		cir.setReturnValue(new PlayerSkin(skin.body(), texture, texture, skin.model(), skin.secure()));
	}
}
