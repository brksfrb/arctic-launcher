//#if MC >= 1.21.2
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ItemPhysicsState;
import net.minecraft.client.renderer.entity.state.ItemEntityRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** See {@link ItemPhysicsState}. */
@Mixin(ItemEntityRenderState.class)
abstract class ItemEntityRenderStateMixin implements ItemPhysicsState {
	@Unique
	private int arctic$place;

	@Override
	public int arctic$place() {
		return arctic$place;
	}

	@Override
	public void arctic$setPlace(int place) {
		arctic$place = place;
	}

	@Unique
	private float arctic$tilt;

	@Override
	public float arctic$tilt() {
		return arctic$tilt;
	}

	@Override
	public void arctic$setTilt(float tilt) {
		arctic$tilt = tilt;
	}
}
//#endif
