package com.arcticlauncher.mod;

/**
 * Polonium (the crowd performance mod) makes other players' render states
 * in full once a tick and only brings them up to the frame between; it calls
 * this after each bringing up, for Arctic's own per-frame work on them: the
 * speaker on a talking player's name tag. A plain JDK interface, so neither
 * mod depends on the other.
 */
public final class PoloniumLightState implements java.util.function.BiConsumer<Object, Object> {
	@Override
	public void accept(Object entity, Object state) {
		//#if MC >= 26.1
		if (entity instanceof net.minecraft.world.entity.Avatar avatar
				&& state instanceof net.minecraft.client.renderer.entity.state.AvatarRenderState avatarState) {
			com.arcticlauncher.mod.cosmetic.SpeakerBadge.apply(avatar, avatarState);
		}
		//#endif
	}
}
