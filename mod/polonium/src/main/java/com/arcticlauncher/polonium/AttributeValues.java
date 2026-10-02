package com.arcticlauncher.polonium;

import java.util.concurrent.atomic.AtomicLong;
import net.minecraft.core.Holder;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.Attributes;

/**
 * A few attributes read for every entity every frame (its scale, how far
 * its name tag shows), kept per entity as values. Looking them up means a
 * map and an attribute object per entity, a frame; with thousands of players
 * that's a noticeable share of making their render states. They hardly ever
 * change: any change to one of them, on any entity, starts a new version,
 * and every entity's kept values are read again.
 */
public final class AttributeValues {
	private static final AtomicLong VERSION = new AtomicLong();

	/** Kept values on an entity (added to LivingEntity by a mixin). */
	public interface Holder2 {
		long polonium$version();

		void polonium$version(long version);

		double polonium$nameDistance();

		double polonium$belowNameDistance();

		void polonium$nameDistances(double name, double belowName);

		long polonium$scaleVersion();

		float polonium$scale();

		void polonium$scale(float scale, long version);
	}

	private AttributeValues() {}

	/** The current version (kept values from another are stale). */
	public static long version() {
		return VERSION.get();
	}

	/** An attribute instance changed (or was made): if it's one kept here, every kept value is stale. */
	public static void changed(Holder<Attribute> attribute) {
		if (attribute == Attributes.SCALE || attribute == Attributes.NAME_TAG_DISTANCE || attribute == Attributes.BELOW_NAME_DISTANCE) {
			VERSION.incrementAndGet();
		}
	}
}
