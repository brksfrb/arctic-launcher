package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.2 && MC < 1.21.9
import com.arcticlauncher.mod.cosmetic.AvatarIdentity;
import java.util.UUID;
import net.minecraft.client.renderer.entity.state.PlayerRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** See {@link AvatarIdentity}. */
@Mixin(PlayerRenderState.class)
abstract class PlayerRenderStateMixin implements AvatarIdentity {
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

	@Unique
	private com.arcticlauncher.client.looks.Look arctic$look;

	@Override
	public com.arcticlauncher.client.looks.Look arctic$look() {
		return arctic$look;
	}

	@Override
	public void arctic$setLook(com.arcticlauncher.client.looks.Look look) {
		arctic$look = look;
	}

	@Unique
	private long arctic$lookedAt;

	@Override
	public long arctic$lookedAt() {
		return arctic$lookedAt;
	}

	@Override
	public void arctic$lookedAt(long millis) {
		arctic$lookedAt = millis;
	}
}
//#endif
