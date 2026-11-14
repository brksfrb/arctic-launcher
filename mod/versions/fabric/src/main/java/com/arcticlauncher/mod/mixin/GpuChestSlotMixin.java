package com.arcticlauncher.mod.mixin;

//#if MC == 26.2
import com.arcticlauncher.mod.GpuChests;
import net.minecraft.world.level.block.entity.BlockEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Where a chest drawn on the GPU keeps its place in the instance data (see {@link GpuChests}), and
 * where that place is given up: when the block entity is removed or its block changes.
 */
@Mixin(BlockEntity.class)
abstract class GpuChestSlotMixin implements GpuChests.Slot {
	@Unique
	private Object arctic$gpu;
	@Unique
	private boolean arctic$lidOpen;

	@Override
	public Object arctic$gpu() {
		return arctic$gpu;
	}

	@Override
	public void arctic$gpu(Object value) {
		arctic$gpu = value;
	}

	@Override
	public boolean arctic$lidOpen() {
		return arctic$lidOpen;
	}

	@Override
	public void arctic$lidOpen(boolean open) {
		arctic$lidOpen = open;
	}

	@Inject(method = "setRemoved", at = @At("HEAD"))
	private void arctic$gpuRemoved(CallbackInfo ci) {
		if (arctic$gpu != null) {
			GpuChests.gone((BlockEntity) (Object) this);
		}
	}

	@Inject(method = "setBlockState", at = @At("HEAD"))
	private void arctic$gpuBlockChanged(net.minecraft.world.level.block.state.BlockState state, CallbackInfo ci) {
		if (arctic$gpu != null) {
			GpuChests.gone((BlockEntity) (Object) this);
		}
	}
}
//#endif
