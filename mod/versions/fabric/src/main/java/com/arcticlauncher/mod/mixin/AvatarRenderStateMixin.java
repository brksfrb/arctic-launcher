package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.mod.cosmetic.AvatarIdentity;
import java.util.UUID;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** See {@link AvatarIdentity}. */
@Mixin(AvatarRenderState.class)
abstract class AvatarRenderStateMixin implements AvatarIdentity {
	@Unique
	private UUID arctic$uuid;

	@Override
	public UUID arctic$uuid() {
		return arctic$uuid;
	}

	@Override
	public void arctic$setUuid(UUID uuid) {
		arctic$uuid = uuid;
	}
}
//#endif
