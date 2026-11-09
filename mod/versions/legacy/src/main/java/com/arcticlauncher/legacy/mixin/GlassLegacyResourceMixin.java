package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyGlassResources;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.resource.ReloadableResourceManagerImpl;
import net.minecraft.resource.ResourcePack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ReloadableResourceManagerImpl.class)
abstract class GlassLegacyResourceMixin implements LegacyGlassResources.ActivePacks {
	@Unique private List<ResourcePack> arctic$active = new ArrayList<>();
	@Inject(method = "reload", at = @At("HEAD"))
	private void arctic$packs(List<ResourcePack> packs, CallbackInfo ci) {
		arctic$active = new ArrayList<>(packs);
	}
	@Override public List<ResourcePack> arctic$packs() { return arctic$active; }
}
