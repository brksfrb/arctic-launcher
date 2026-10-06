//#if MC >= 26.1
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
}
//#endif
