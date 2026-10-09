package com.arcticlauncher.mod.mixin;

//#if MC < 1.21.9
import com.arcticlauncher.mod.cosmetic.CosmeticsLayer;
import net.minecraft.client.renderer.entity.player.PlayerRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#if MC >= 1.17
import net.minecraft.client.renderer.entity.EntityRendererProvider;
//#else
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
//#endif
//#if MC >= 1.21.2
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.cosmetic.AvatarIdentity;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.renderer.entity.state.PlayerRenderState;
//#endif

/** Players get Arctic's cosmetics layer (before 1.21.9, where players had their own renderer and no Avatar). */
@Mixin(PlayerRenderer.class)
abstract class OldPlayerRendererMixin {
	@SuppressWarnings({"unchecked", "rawtypes"})
	//#if MC >= 1.17
	@Inject(method = "<init>", at = @At("TAIL"))
	//#else
	// Before 1.17 there is also a one-argument constructor (it calls this one).
	@Inject(method = "<init>(Lnet/minecraft/client/renderer/entity/EntityRenderDispatcher;Z)V", at = @At("TAIL"))
	//#endif
	//#if MC >= 1.17
	private void arctic$cosmetics(EntityRendererProvider.Context context, boolean slim, CallbackInfo ci) {
	//#else
	private void arctic$cosmetics(EntityRenderDispatcher dispatcher, boolean slim, CallbackInfo ci) {
	//#endif
		PlayerRenderer renderer = (PlayerRenderer) (Object) this;
		((LivingEntityRendererAccess) this).arctic$addLayer(new CosmeticsLayer(renderer));
	}
	//#if MC >= 1.21.2

	/** How long a player's looks found for its render state stay good. */
	private static final long LOOK_REFRESH_MS = 250;

	/** Who this is, kept on the state (see {@link AvatarIdentity}). */
	@Inject(method = "extractRender" + "State(Lnet/minecraft/client/player/AbstractClientPlayer;Lnet/minecraft/client/renderer/entity/state/PlayerRenderState;F)V",
			at = @At("TAIL"))
	private void arctic$identify(AbstractClientPlayer entity, PlayerRenderState state, float partialTick, CallbackInfo ci) {
		java.util.UUID id = entity.getUUID();
		AvatarIdentity identity = (AvatarIdentity) state;
		long now = System.currentTimeMillis();
		boolean fresh = !id.equals(identity.arctic$uuid()) || now - identity.arctic$lookedAt() > LOOK_REFRESH_MS;
		identity.arctic$setUuid(id);
		if (fresh) {
			identity.arctic$setLook(ArcticClient.looks() != null ? ArcticClient.looks().lookFor(id) : null);
			identity.arctic$lookedAt(now);
			if (ArcticClient.looks() != null) {
				ArcticClient.looks().cosmetics().watch(id);
			}
		}
	}
	//#endif
}
//#endif
