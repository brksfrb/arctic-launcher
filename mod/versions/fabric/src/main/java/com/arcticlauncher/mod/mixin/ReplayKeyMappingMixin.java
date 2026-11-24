//#if MC >= 1.13
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.Replays;
import net.minecraft.client.KeyMapping;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#if MC < 1.21.9
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.mojang.blaze3d.platform.InputConstants;
import java.util.Map;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#endif

/** Viewer-only keys are dispatched by Arctic so they never replace a vanilla action on the same key. */
@Mixin(KeyMapping.class)
abstract class ReplayKeyMappingMixin {
	//#if MC >= 1.21.9
	@Inject(method = "registerMapping", at = @At("HEAD"), cancellable = true)
	private void arctic$viewerOnly(CallbackInfo ci) {
		if (Replays.viewerBinding(((KeyMapping) (Object) this).getName())) ci.cancel();
	}
	//#else
	@WrapOperation(method = "<init>(Ljava/lang/String;Lcom/mojang/blaze3d/platform/InputConstants$Type;ILjava/lang/String;)V",
			at = @At(value = "INVOKE", target = "Ljava/util/Map;put(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;"))
	private Object arctic$viewerOnlyAtRegistration(Map<?, ?> map, Object key, Object value, Operation<Object> original) {
		return key instanceof InputConstants.Key && Replays.viewerBinding(((KeyMapping) value).getName())
				? null : original.call(map, key, value);
	}

	@WrapOperation(method = "resetMapping",
			at = @At(value = "INVOKE", target = "Ljava/util/Map;put(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;"))
	private static Object arctic$viewerOnlyOnReset(Map<?, ?> map, Object key, Object value, Operation<Object> original) {
		return Replays.viewerBinding(((KeyMapping) value).getName()) ? null : original.call(map, key, value);
	}

	@Inject(method = "compareTo(Lnet/minecraft/client/KeyMapping;)I", at = @At("HEAD"), cancellable = true)
	private void arctic$sortViewerKeys(KeyMapping other, CallbackInfoReturnable<Integer> cir) {
		boolean ours = Replays.viewerBinding(((KeyMapping) (Object) this).getName());
		boolean theirs = Replays.viewerBinding(other.getName());
		if (ours != theirs) cir.setReturnValue(ours ? -1 : 1);
	}
	//#endif
}
//#endif
