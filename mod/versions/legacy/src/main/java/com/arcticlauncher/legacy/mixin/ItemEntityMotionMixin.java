package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.ItemPhysicsMotion;
import net.minecraft.entity.ItemEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** See {@link ItemPhysicsMotion}. */
@Mixin(ItemEntity.class)
abstract class ItemEntityMotionMixin implements ItemPhysicsMotion {
	@Unique
	private float arctic$tilt;
	@Unique
	private long arctic$tiltAt;

	@Override
	public float arctic$tilt() {
		return arctic$tilt;
	}

	@Override
	public long arctic$tiltAt() {
		return arctic$tiltAt;
	}

	@Override
	public void arctic$setTilt(float tilt, long at) {
		arctic$tilt = tilt;
		arctic$tiltAt = at;
	}

	@Unique
	private double arctic$airY;
	@Unique
	private long arctic$airAt;

	@Override
	public double arctic$airY() {
		return arctic$airY;
	}

	@Override
	public long arctic$airAt() {
		return arctic$airAt;
	}

	@Override
	public void arctic$setAir(double y, long at) {
		arctic$airY = y;
		arctic$airAt = at;
	}
}
