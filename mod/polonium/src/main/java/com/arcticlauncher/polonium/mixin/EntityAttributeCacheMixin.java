package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.AttributeCache;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** Room on each entity for two remembered attributes (see {@link AttributeCache}). */
@Mixin(Entity.class)
abstract class EntityAttributeCacheMixin implements AttributeCache.Holder2 {
	@Unique
	private final Object[] polonium$attributes = new Object[4];

	@Override
	public Object[] polonium$attributes() {
		return polonium$attributes;
	}
}
