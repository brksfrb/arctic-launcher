package com.arcticlauncher.mod.mixin;

//#if MC == 26.2
import com.arcticlauncher.mod.GpuChests;
import net.minecraft.client.multiplayer.ClientLevel;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A section and its neighbours are to be drawn afresh (light sent by the server, say): their chests' light is read again. */
@Mixin(ClientLevel.class)
abstract class GpuChestDirtyMixin {
	@Inject(method = "setSectionDirtyWithNeighbors", at = @At("HEAD"))
	private void arctic$chestsDirty(int sectionX, int sectionY, int sectionZ, CallbackInfo ci) {
		GpuChests.lightChanged(sectionX, sectionY, sectionZ);
	}
}
//#endif
