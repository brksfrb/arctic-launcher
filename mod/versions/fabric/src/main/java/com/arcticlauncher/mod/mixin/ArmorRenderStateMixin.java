//#if MC >= 1.21.2
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ArmorVisibility;
import net.minecraft.client.renderer.entity.state.HumanoidRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

@Mixin(HumanoidRenderState.class)
abstract class ArmorRenderStateMixin implements ArmorVisibility {
	@Unique
	private boolean arctic$localPlayer;

	@Override
	public boolean arctic$isLocalPlayer() {
		return arctic$localPlayer;
	}

	@Override
	public void arctic$setLocalPlayer(boolean local) {
		arctic$localPlayer = local;
	}
}
//#endif
