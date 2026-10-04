package com.arcticlauncher.mod;

/**
 * Polarium (the crowd performance mod) works most players' poses out
 * directly, skipping the model's setupAnim; it asks this whether Arctic
 * poses a player this frame (an emote playing), so those still go through
 * setupAnim and its emote hook (PlayerModelEmoteMixin). A plain JDK
 * interface, so neither mod depends on the other. Called on Polarium's
 * helper threads: only reads.
 */
public final class PolariumPosing implements java.util.function.Predicate<Object> {
	@Override
	public boolean test(Object state) {
		//#if MC >= 26.1
		if (!(state instanceof net.minecraft.client.renderer.entity.state.AvatarRenderState avatar)
				|| com.arcticlauncher.client.ArcticClient.looks() == null) {
			return false;
		}
		java.util.UUID id = com.arcticlauncher.mod.cosmetic.CosmeticsLayer.player(avatar);
		return id != null && com.arcticlauncher.client.ArcticClient.looks().cosmetics().playingFor(id) != null;
		//#else
		return false;
		//#endif
	}
}
