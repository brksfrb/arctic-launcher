package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.AttributeValues;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** An entity's kept scale and name tag distances (see {@link AttributeValues}). */
@Mixin(LivingEntity.class)
abstract class LivingEntityAttributeValuesMixin implements AttributeValues.Holder2 {
	@Unique
	private long polonium$version = -1;
	@Unique
	private double polonium$nameDistance;
	@Unique
	private double polonium$belowNameDistance;
	@Unique
	private long polonium$scaleVersion = -1;
	@Unique
	private float polonium$scale;

	@Override
	public long polonium$version() {
		return polonium$version;
	}

	@Override
	public void polonium$version(long version) {
		polonium$version = version;
	}

	@Override
	public double polonium$nameDistance() {
		return polonium$nameDistance;
	}

	@Override
	public double polonium$belowNameDistance() {
		return polonium$belowNameDistance;
	}

	@Override
	public void polonium$nameDistances(double name, double belowName) {
		polonium$nameDistance = name;
		polonium$belowNameDistance = belowName;
	}

	@Override
	public long polonium$scaleVersion() {
		return polonium$scaleVersion;
	}

	@Override
	public float polonium$scale() {
		return polonium$scale;
	}

	@Override
	public void polonium$scale(float scale, long version) {
		polonium$scale = scale;
		polonium$scaleVersion = version;
	}

	@WrapMethod(method = "getScale")
	private float polonium$keptScale(Operation<Float> scale) {
		long version = AttributeValues.version();
		if (polonium$scaleVersion == version) {
			return polonium$scale;
		}
		float value = scale.call();
		polonium$scale(value, version);
		return value;
	}
}
