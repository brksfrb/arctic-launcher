package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.gpu.CrowdChecked;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** Room on a player's render state for the crowd path's check (see {@link CrowdChecked}). */
@Mixin(AvatarRenderState.class)
abstract class AvatarRenderStateCrowdMixin implements CrowdChecked {
	@Unique
	private Object polonium$checked;
	@Unique
	private long polonium$checkedFrame = -1;

	@Override
	public Object polonium$checked() {
		return polonium$checked;
	}

	@Override
	public long polonium$checkedFrame() {
		return polonium$checkedFrame;
	}

	@Unique
	private boolean polonium$liveLayers = true;

	@Override
	public boolean polonium$liveLayers() {
		return polonium$liveLayers;
	}

	@Override
	public void polonium$liveLayers(boolean live) {
		polonium$liveLayers = live;
	}

	@Override
	public void polonium$checked(Object recipe, long frame) {
		polonium$checked = recipe;
		polonium$checkedFrame = frame;
	}
}
