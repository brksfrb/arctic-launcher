package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyGlassResources;
import net.minecraft.resource.AbstractFileResourcePack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(AbstractFileResourcePack.class)
public interface GlassLegacyPackAccess extends LegacyGlassResources.FilePack {
	@Accessor("base") java.io.File arctic$root();
}
