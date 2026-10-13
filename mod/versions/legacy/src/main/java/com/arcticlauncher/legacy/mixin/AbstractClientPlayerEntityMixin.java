package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Look;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.util.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Shows the player's Arctic look: skin (with arm model) and cape.
 *
 * <p>Legacy Yarn has these two names the wrong way round (1.8.9 to 1.12.2): {@code getCapeId} is the
 * skin (the player renderer draws the body with it) and {@code getSkinId} is the cape (the cape layer
 * draws with it). See {@link com.arcticlauncher.legacy.LegacyHooks#skinOf}.
 */
@Mixin(AbstractClientPlayerEntity.class)
abstract class AbstractClientPlayerEntityMixin {
	/** The skin, despite its name. */
	@Inject(method = "getCapeId", at = @At("RETURN"), cancellable = true)
	private void arctic$skin(CallbackInfoReturnable<Identifier> cir) {
		if (LegacyHooks.streamerSelf(this)) {
			cir.setReturnValue(LegacyHooks.STEVE);
			return;
		}
		Look look = look();
		Identifier tex = look == null ? null : LegacyHooks.ready(look.skin);
		if (tex != null) {
			cir.setReturnValue(tex);
		}
	}

	/** The cape, despite its name. */
	@Inject(method = "getSkinId()Lnet/minecraft/util/Identifier;", at = @At("RETURN"), cancellable = true)
	private void arctic$cape(CallbackInfoReturnable<Identifier> cir) {
		Look look = look();
		Identifier tex = look == null ? null : LegacyHooks.ready(look.cape);
		if (tex != null) {
			cir.setReturnValue(tex);
		}
	}

	@Inject(method = "getModel", at = @At("RETURN"), cancellable = true)
	private void arctic$model(CallbackInfoReturnable<String> cir) {
		Look look = look();
		if (look != null && LegacyHooks.ready(look.skin) != null) {
			cir.setReturnValue(look.slim ? "slim" : "default");
		}
	}

	private Look look() {
		return ArcticClient.looks() == null ? null
				: ArcticClient.looks().lookFor(((AbstractClientPlayerEntity) (Object) this).getUuid());
	}
}
