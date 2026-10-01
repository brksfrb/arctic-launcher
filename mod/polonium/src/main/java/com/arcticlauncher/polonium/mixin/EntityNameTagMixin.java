package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.NameTags;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** Room on each entity for its kept name tag text (see {@link NameTags}). */
@Mixin(Entity.class)
abstract class EntityNameTagMixin implements NameTags.Holder {
	@Unique
	private NameTags.Kept polonium$nameTag;

	@Override
	public NameTags.Kept polonium$nameTag() {
		return polonium$nameTag;
	}

	@Override
	public void polonium$nameTag(NameTags.Kept kept) {
		polonium$nameTag = kept;
	}
}
