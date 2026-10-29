//#if MC >= 1.21.9
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.DebugListRebuild;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.gui.components.debug.DebugScreenEntries;
import net.minecraft.client.gui.components.debug.DebugScreenEntryList;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Coerce;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#if MC >= 1.21.11
import java.util.List;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Shadow;
//#endif

/**
 * The Hitboxes mod: the game draws entity hitboxes while its debug list says they are on. Arctic's switch
 * says so without touching the player's own debug profile (which is saved to disk).
 */
@Mixin(DebugScreenEntryList.class)
abstract class DebugEntriesMixin implements DebugListRebuild {
	//#if MC >= 1.21.11
	@Shadow
	@Final
	@SuppressWarnings("rawtypes")
	private List currentlyEnabled;

	@Shadow
	protected abstract void rebuildCurrentList();

	@Override
	public void arctic$rebuild() {
		rebuildCurrentList();
	}

	/** The list the game reads each frame carries Arctic's entry, so a switch takes effect at once, not after a rejoin. */
	@Inject(method = "rebuildCurrentList", at = @At("TAIL"), require = 0)
	@SuppressWarnings({"rawtypes", "unchecked"})
	private void arctic$addEntries(CallbackInfo ci) {
		ClientConfig config = ArcticClient.config();
		if (config != null && config.hitboxes && !currentlyEnabled.contains(DebugScreenEntries.ENTITY_HITBOXES)) {
			currentlyEnabled.add(DebugScreenEntries.ENTITY_HITBOXES);
		}
	}
	//#else
	@Override
	public void arctic$rebuild() {}
	//#endif

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
