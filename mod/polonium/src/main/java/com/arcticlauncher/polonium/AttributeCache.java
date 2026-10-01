package com.arcticlauncher.polonium;

import net.minecraft.core.Holder;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;

/**
 * An entity's name tag distances, looked up every frame for every entity
 * (two attribute lookups each). The attribute objects stay the same for the
 * entity's life (their values change in place), so each entity remembers the
 * two it was asked for and reads their current value. Render thread only.
 */
public final class AttributeCache {
	private AttributeCache() {}

	/** Holder of an entity's two remembered attributes (added to Entity by a mixin). */
	public interface Holder2 {
		Object[] polonium$attributes();
	}

	/** The entity's attribute, remembered after the first lookup. */
	public static AttributeInstance get(LivingEntity entity, Holder<Attribute> attribute,
			java.util.function.Supplier<AttributeInstance> lookUp) {
		Object[] slots = ((Holder2) entity).polonium$attributes();
		for (int i = 0; i < slots.length; i += 2) {
			if (slots[i] == attribute) {
				return (AttributeInstance) slots[i + 1];
			}
		}
		AttributeInstance instance = lookUp.get();
		if (instance != null) {
			for (int i = 0; i < slots.length; i += 2) {
				if (slots[i] == null) {
					slots[i] = attribute;
					slots[i + 1] = instance;
					break;
				}
			}
		}
		return instance;
	}
}
