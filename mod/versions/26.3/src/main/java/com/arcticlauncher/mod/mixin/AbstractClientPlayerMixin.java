package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Look;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.core.ClientAsset;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.player.PlayerModelType;
import net.minecraft.world.entity.player.PlayerSkin;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Shows the player's Arctic look: skin (with arm model) and cape/elytra. */
@Mixin(AbstractClientPlayer.class)
abstract class AbstractClientPlayerMixin {
	@Inject(method = "getSkin", at = @At("RETURN"), cancellable = true)
	private void arctic$look(CallbackInfoReturnable<PlayerSkin> cir) {
		Look look = ArcticClient.looks().lookFor(((AbstractClientPlayer) (Object) this).getUUID());
		if (look == null) {
			return;
		}
		PlayerSkin skin = cir.getReturnValue();
		ClientAsset.Texture body = skin.body();
		PlayerModelType model = skin.model();
		Identifier skinTexture = ready(look.skin);
		if (skinTexture != null) {
			body = new ClientAsset.ResourceTexture(skinTexture, skinTexture);
			model = look.slim ? PlayerModelType.SLIM : PlayerModelType.WIDE;
		}
		ClientAsset.Texture cape = skin.cape();
		ClientAsset.Texture elytra = skin.elytra();
		Identifier capeTexture = ready(look.cape);
		if (capeTexture != null) {
			cape = new ClientAsset.ResourceTexture(capeTexture, capeTexture);
			elytra = cape;
		}
		cir.setReturnValue(new PlayerSkin(body, cape, elytra, model, skin.secure()));
	}

	private static Identifier ready(String hash) {
		return ArcticClient.looks().texture(hash) ? GfxImpl.look(hash) : null;
	}
}
