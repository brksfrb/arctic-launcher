package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.feature.ChatCopy;
import net.minecraft.text.Style;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Legacy setters (including color changes) mutate this style.
 * Only copy/deepCopy allocate a replacement, so both must snapshot ownership.
 */
@Mixin(Style.class)
abstract class ChatCopyStyleMixin implements ChatCopy.Tagged {
	@Shadow private Style parent;
	@Unique private ChatCopy.Data arctic$copyData;
	public ChatCopy.Data arctic$copyData() {
		return arctic$copyData != null ? arctic$copyData : ChatCopy.data(parent);
	}
	public void arctic$copyData(ChatCopy.Data data) { arctic$copyData = data; }

	@Inject(method = {"copy", "deepCopy"}, at = @At("RETURN"))
	private void arctic$inherit(CallbackInfoReturnable<Style> cir) {
		((ChatCopy.Tagged) cir.getReturnValue()).arctic$copyData(arctic$copyData());
	}
	// Resolve at hover time so already received messages follow the current settings.
	@Inject(method = "getHoverEvent", at = @At("HEAD"), cancellable = true)
	private void arctic$hover(CallbackInfoReturnable<net.minecraft.text.HoverEvent> cir) {
		ChatCopy.Data data = arctic$copyData();
		if (data == null || !data.target) return;
		if (!ChatCopy.enabled()) {
			cir.setReturnValue(null);
			return;
		}
		cir.setReturnValue(new net.minecraft.text.HoverEvent(net.minecraft.text.HoverEvent.Action.SHOW_TEXT,
				new net.minecraft.text.LiteralText("Click to copy")));
	}

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
