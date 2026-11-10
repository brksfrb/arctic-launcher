//#if MC < 1.16
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.GlassResources;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.server.packs.Pack;
import net.minecraft.server.packs.resources.SimpleReloadableResourceManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Older managers expose pack ownership, but not the ordered list of packs. */
@Mixin(SimpleReloadableResourceManager.class)
abstract class GlassPackOrderMixin implements GlassResources.ActivePacks {
	@Unique private final List<String> arctic$packNames = new ArrayList<>();
	@Inject(method = "add", at = @At("TAIL"))
	private void arctic$add(Pack pack, CallbackInfo ci) {
		arctic$packNames.add(pack.getName());
	}
	@Inject(method = "clear", at = @At("HEAD"))
	private void arctic$clear(CallbackInfo ci) {
		arctic$packNames.clear();
	}
	@Override public List<String> arctic$packNames() { return arctic$packNames; }
}
//#endif
