package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.legacy.LegacyKeys;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.util.collection.IntObjectStorage;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Every key and mouse press while playing (no screen) passes here (mouse
 * buttons as -100 + n): clicks count for CPS, Right Shift opens the menu.
 */
@Mixin(KeyBinding.class)
abstract class KeyBindingMixin {
	private static final int MOUSE_OFFSET = -100;

	@Inject(method = "onKeyPressed", at = @At("HEAD"), cancellable = true)
	private static void arctic$press(int code, CallbackInfo ci) {
		if (code < 0) {
			int button = LegacyKeys.coreMouse(code - MOUSE_OFFSET);
			if (MinecraftClient.getInstance().currentScreen == null && Replays.key(button, LegacyKeys.name(code))) ci.cancel();
			else ArcticClient.mousePressed(button);
			return;
		}
		if (MinecraftClient.getInstance().currentScreen == null && ArcticClient.keyPressed(LegacyKeys.core(code), code)) {
			LegacyKeys.used(code, org.lwjgl.input.Keyboard.getEventNanoseconds());
			ci.cancel();
		}
	}

	@WrapOperation(method = "<init>(Ljava/lang/String;ILjava/lang/String;)V",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/util/collection/IntObjectStorage;set(ILjava/lang/Object;)V"))
	private void arctic$viewerOnlyAtRegistration(IntObjectStorage<?> keys, int code, Object binding, Operation<Void> original) {
		if (!Replays.viewerBinding(((KeyBinding) binding).getTranslationKey())) original.call(keys, code, binding);
	}

	@WrapOperation(method = "updateKeysByCode",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/util/collection/IntObjectStorage;set(ILjava/lang/Object;)V"))
	private static void arctic$viewerOnlyOnReset(IntObjectStorage<?> keys, int code, Object binding, Operation<Void> original) {
		if (!Replays.viewerBinding(((KeyBinding) binding).getTranslationKey())) original.call(keys, code, binding);
	}

	@Inject(method = "compareTo(Lnet/minecraft/client/option/KeyBinding;)I", at = @At("HEAD"), cancellable = true)
	private void arctic$sortViewerKeys(KeyBinding other, CallbackInfoReturnable<Integer> cir) {
		boolean ours = Replays.viewerBinding(((KeyBinding) (Object) this).getTranslationKey());
		boolean theirs = Replays.viewerBinding(other.getTranslationKey());
		if (ours != theirs) cir.setReturnValue(ours ? -1 : 1);
	}
}
