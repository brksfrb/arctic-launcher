package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.NameTagCache;
import net.minecraft.network.chat.MutableComponent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** Room on each text for its kept name tag layouts (see {@link NameTagCache}). */
@Mixin(MutableComponent.class)
abstract class ComponentLayoutMixin implements NameTagCache.Holder {
	@Unique
	private NameTagCache.Kept polonium$layouts;

	@Override
	public NameTagCache.Kept polonium$layouts() {
		return polonium$layouts;
	}

	@Override
	public void polonium$layouts(NameTagCache.Kept kept) {
		polonium$layouts = kept;
	}
}
