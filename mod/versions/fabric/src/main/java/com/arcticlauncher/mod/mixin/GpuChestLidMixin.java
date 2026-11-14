package com.arcticlauncher.mod.mixin;

//#if MC == 26.2
import com.arcticlauncher.mod.GpuChests;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.ChestBlockEntity;
import net.minecraft.world.level.block.entity.EnderChestBlockEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A chest's lid is told to open or shut: from then until it has shut again it is drawn with its lid's angle (see {@link GpuChests}). */
@Mixin({ChestBlockEntity.class, EnderChestBlockEntity.class})
abstract class GpuChestLidMixin {
	@Inject(method = "triggerEvent", at = @At("HEAD"))
	private void arctic$lidEvent(int event, int count, CallbackInfoReturnable<Boolean> cir) {
		GpuChests.lidEvent((BlockEntity) (Object) this, event, count);
	}
}
//#endif
