package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Look;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.resources.Identifier;
//#if MC >= 1.21.9
import net.minecraft.core.ClientAsset;
import net.minecraft.world.entity.player.PlayerModelType;
import net.minecraft.world.entity.player.PlayerSkin;
//#else
import net.minecraft.client.resources.PlayerSkin;
//#endif
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
		if (look != null) {
			cir.setReturnValue(withLook(cir.getReturnValue(), look));
		}
	}

	//#if MC >= 1.21.9
	private static PlayerSkin withLook(PlayerSkin skin, Look look) {
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
		return new PlayerSkin(body, cape, elytra, model, skin.secure());
	}
	//#else
	private static PlayerSkin withLook(PlayerSkin skin, Look look) {
		Identifier body = skin.texture();
		PlayerSkin.Model model = skin.model();
		Identifier skinTexture = ready(look.skin);
		if (skinTexture != null) {
			body = skinTexture;
			model = look.slim ? PlayerSkin.Model.SLIM : PlayerSkin.Model.WIDE;
		}
		Identifier cape = skin.capeTexture();
		Identifier elytra = skin.elytraTexture();
		Identifier capeTexture = ready(look.cape);
		if (capeTexture != null) {
			cape = capeTexture;
			elytra = capeTexture;
		}
		return new PlayerSkin(body, skin.textureUrl(), cape, elytra, model, skin.secure());
	}
	//#endif

	private static Identifier ready(String hash) {
		// Animated capes show the current frame.
		return ArcticClient.looks().texture(hash) ? GfxImpl.look(ArcticClient.looks().frame(hash)) : null;
	}
}
