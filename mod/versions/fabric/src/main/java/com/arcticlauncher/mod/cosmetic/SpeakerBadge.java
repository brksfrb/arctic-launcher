package com.arcticlauncher.mod.cosmetic;

//#if MC >= 26.1
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.voice.VoiceLink;
import com.arcticlauncher.mod.Compat;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.Avatar;
//#endif

/** A speaker in front of a talking player's name tag (each frame its render state is made). */
public final class SpeakerBadge {
	private SpeakerBadge() {}

	//#if MC >= 26.1
	public static void apply(Avatar entity, AvatarRenderState state) {
		VoiceLink voice = ArcticClient.voice();
		if (state.nameTag == null || voice == null || !voice.active() || entity == null) {
			return;
		}
		if (voice.isSpeaking(entity.getUUID())) {
			Component speaker = Compat.speakerBadge();
			Component gap = Compat.literal(" ").withStyle(Compat.arcticBadge().getStyle());
			state.nameTag = Compat.empty().append(speaker).append(gap).append(state.nameTag);
		}
	}
	//#endif
}
