package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.feature.ChatCopy;
import net.minecraft.network.chat.Style;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Preserve message ownership when vanilla resolves inherited styles during wrapping. */
@Mixin(Style.class)
abstract class ChatCopyStyleMixin implements ChatCopy.Tagged {
	//#if MC < 1.16
	@org.spongepowered.asm.mixin.Shadow private Style parent;
	//#endif
	@Unique private ChatCopy.Data arctic$copyData;
	public ChatCopy.Data arctic$copyData() {
		//#if MC >= 1.16
		return arctic$copyData;
		//#else
		return arctic$copyData != null ? arctic$copyData : ChatCopy.data(parent);
		//#endif
	}
	public void arctic$copyData(ChatCopy.Data data) { arctic$copyData = data; }

	//#if MC >= 1.16
	@Inject(method = "applyTo", at = @At("RETURN"))
	private void arctic$inherit(Style parent, CallbackInfoReturnable<Style> cir) {
		ChatCopy.Data data = arctic$copyData != null ? arctic$copyData : ChatCopy.data(parent);
		if (data != null && cir.getReturnValue() != Style.EMPTY) {
			((ChatCopy.Tagged) (Object) cir.getReturnValue()).arctic$copyData(data);
		}
	}
	//#else
	@Inject(method = {"copy", "flatCopy"}, at = @At("RETURN"))
	private void arctic$inherit(CallbackInfoReturnable<Style> cir) {
		((ChatCopy.Tagged) (Object) cir.getReturnValue()).arctic$copyData(arctic$copyData());
	}
	//#endif
	@Inject(method = "equals", at = @At("RETURN"), cancellable = true)
	private void arctic$equal(Object other, CallbackInfoReturnable<Boolean> cir) {
		if (cir.getReturnValue() && !java.util.Objects.equals(arctic$copyData(), ChatCopy.data(other))) {
			cir.setReturnValue(false);
		}
	}

	@Inject(method = "hashCode", at = @At("RETURN"), cancellable = true)
	private void arctic$hash(CallbackInfoReturnable<Integer> cir) {
		ChatCopy.Data data = arctic$copyData();
		if (data != null) cir.setReturnValue(31 * cir.getReturnValue() + data.hashCode());
	}
}
