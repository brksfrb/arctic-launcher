package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.voice.VoiceLink;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.cosmetic.AvatarIdentity;
import com.arcticlauncher.mod.cosmetic.CosmeticsLayer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.Avatar;
import net.minecraft.client.renderer.entity.EntityRendererProvider;
import net.minecraft.client.renderer.entity.player.AvatarRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Players get Arctic's cosmetics layer, and a speaker on their name tag while talking. */
@Mixin(AvatarRenderer.class)
abstract class AvatarRendererMixin {
	/** How long a player's looks found for its render state stay good. */
	private static final long LOOK_REFRESH_MS = 250;

	@SuppressWarnings({"unchecked", "rawtypes"})
	@Inject(method = "<init>", at = @At("TAIL"))
	private void arctic$cosmetics(EntityRendererProvider.Context context, boolean slim, CallbackInfo ci) {
		AvatarRenderer renderer = (AvatarRenderer) (Object) this;
		((LivingEntityRendererAccess) this).arctic$addLayer(new CosmeticsLayer(renderer));
	}

	/** Who this is, kept on the state (see {@link AvatarIdentity}), and seen for emote updates. */
	@Inject(method = "extractRenderState(Lnet/minecraft/world/entity/Avatar;Lnet/minecraft/client/renderer/entity/state/AvatarRenderState;F)V",
			at = @At("TAIL"))
	private void arctic$identify(Avatar entity, AvatarRenderState state, float partialTick, CallbackInfo ci) {
		// Only players can be Arctic players: mannequins and other player-shaped
		// entities skip the looks, emotes and cosmetics work (a crowd of them adds up).
		java.util.UUID id = entity instanceof net.minecraft.world.entity.player.Player ? entity.getUUID() : null;
		AvatarIdentity identity = (AvatarIdentity) state;
		long now = System.currentTimeMillis();
		// Performance mods may keep a player's state from frame to frame: then its looks needn't be looked up every frame.
		boolean fresh = id == null || !id.equals(identity.arctic$uuid()) || now - identity.arctic$lookedAt() > LOOK_REFRESH_MS;
		identity.arctic$setUuid(id);
		if (fresh) {
			identity.arctic$setLook(id != null && ArcticClient.looks() != null ? ArcticClient.looks().lookFor(id) : null);
			identity.arctic$lookedAt(now);
			if (id != null && ArcticClient.looks() != null) {
				ArcticClient.looks().cosmetics().watch(id);
			}
		}
	}

	/** A speaker before the name of a player talking in voice chat. */
	@Inject(method = "extractRenderState(Lnet/minecraft/world/entity/Avatar;Lnet/minecraft/client/renderer/entity/state/AvatarRenderState;F)V",
			at = @At("TAIL"))
	private void arctic$speaking(Avatar entity, AvatarRenderState state, float partialTick, CallbackInfo ci) {
		VoiceLink voice = ArcticClient.voice();
		if (state.nameTag == null || voice == null || !voice.active() || entity == null) {
			return;
		}
		if (voice.isSpeaking(entity.getUUID())) {
			Component speaker = Compat.speakerBadge();
			Component gap = com.arcticlauncher.mod.Compat.literal(" ").withStyle(Compat.arcticBadge().getStyle());
			state.nameTag = com.arcticlauncher.mod.Compat.empty().append(speaker).append(gap).append(state.nameTag);
		}
	}
}
//#endif
