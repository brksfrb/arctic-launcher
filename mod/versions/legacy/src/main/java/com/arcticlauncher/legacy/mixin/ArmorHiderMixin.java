package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.feature.ArmorHider;
import com.arcticlauncher.client.feature.ArmorHider.Piece;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.entity.feature.ArmorFeatureRenderer;
//#if MC >= 1.9
import net.minecraft.entity.EquipmentSlot;
//#endif
import net.minecraft.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ArmorFeatureRenderer.class)
abstract class ArmorHiderMixin {
	//#if MC >= 1.9
	@Inject(method = "method_10278", at = @At("HEAD"), cancellable = true)
	private void arctic$hide(LivingEntity entity, float a, float b, float c, float d, float e, float f,
			float scale, EquipmentSlot slot, CallbackInfo ci) {
		if (entity != MinecraftClient.getInstance().player) return;
		Piece piece;
		switch (slot) {
			case HEAD: piece = Piece.HELMET; break;
			case CHEST: piece = Piece.CHESTPLATE; break;
			case LEGS: piece = Piece.LEGGINGS; break;
			case FEET: piece = Piece.BOOTS; break;
			default: return;
		}
		if (ArmorHider.hides(piece)) ci.cancel();
	}
	//#else
	@Inject(method = "renderFeature", at = @At("HEAD"), cancellable = true)
	private void arctic$hide(LivingEntity entity, float a, float b, float c, float d, float e, float f,
			float scale, int slot, CallbackInfo ci) {
		if (entity != MinecraftClient.getInstance().player) return;
		Piece piece;
		switch (slot) {
			case 4: piece = Piece.HELMET; break;
			case 3: piece = Piece.CHESTPLATE; break;
			case 2: piece = Piece.LEGGINGS; break;
			case 1: piece = Piece.BOOTS; break;
			default: return;
		}
		if (ArmorHider.hides(piece)) ci.cancel();
	}
	//#endif
}
