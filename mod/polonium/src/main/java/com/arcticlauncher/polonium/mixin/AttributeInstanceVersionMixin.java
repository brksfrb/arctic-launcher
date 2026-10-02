package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.AttributeValues;
import java.util.function.Consumer;
import net.minecraft.core.Holder;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Kept attribute values go stale when one of their attributes changes anywhere (see {@link AttributeValues}). */
@Mixin(AttributeInstance.class)
abstract class AttributeInstanceVersionMixin {
	@Shadow
	@Final
	private Holder<Attribute> attribute;

	@Inject(method = "<init>", at = @At("TAIL"))
	private void polonium$made(Holder<Attribute> attribute, Consumer<AttributeInstance> onDirty, CallbackInfo ci) {
		AttributeValues.changed(attribute);
	}

	@Inject(method = "setDirty", at = @At("HEAD"))
	private void polonium$changed(CallbackInfo ci) {
		AttributeValues.changed(this.attribute);
	}
}
