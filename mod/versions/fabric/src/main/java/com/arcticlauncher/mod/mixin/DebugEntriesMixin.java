//#if MC >= 1.21.9
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.gui.components.debug.DebugScreenEntries;
import net.minecraft.client.gui.components.debug.DebugScreenEntryList;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Coerce;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The Hitboxes mod: the game draws entity hitboxes while its debug list says they are on. Arctic's switch
 * says so without touching the player's own debug profile (which is saved to disk).
 */
@Mixin(DebugScreenEntryList.class)
abstract class DebugEntriesMixin {
	@Inject(method = "isCurrentlyEnabled", at = @At("HEAD"), cancellable = true, require = 0)
	// The entry's class is ResourceLocation or Identifier by version: @Coerce takes either.
	private void arctic$hitboxes(@Coerce Object entry, CallbackInfoReturnable<Boolean> cir) {
		ClientConfig config = ArcticClient.config();
		if (config != null && config.hitboxes && DebugScreenEntries.ENTITY_HITBOXES.equals(entry)) {
			cir.setReturnValue(true);
		}
	}
}
//#endif
