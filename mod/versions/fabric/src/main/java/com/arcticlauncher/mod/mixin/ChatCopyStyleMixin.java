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
	@Inject(method = "applyTo", at = @At("RETURN"), cancellable = true)
	private void arctic$inherit(Style parent, CallbackInfoReturnable<Style> cir) {
		ChatCopy.Data data = arctic$copyData != null ? arctic$copyData : ChatCopy.data(parent);
		arctic$tag(cir, data);
	}
	@Inject(method = {"withColor", "withBold", "withItalic", "withUnderlined",
			"withClickEvent", "withHoverEvent", "withInsertion", "withFont",
			"applyFormat", "applyLegacyFormat", "applyFormats"
			//#if MC >= 1.17
			, "withStrikethrough", "withObfuscated"
			//#endif
			//#if MC >= 1.21.4
			, "withShadowColor"
			//#endif
			//#if MC >= 1.21.11
			, "withoutShadow"
			//#endif
	}, at = @At("RETURN"), cancellable = true)
	private void arctic$transform(CallbackInfoReturnable<Style> cir) {
		arctic$tag(cir, arctic$copyData);
	}

	@Unique
	private void arctic$tag(CallbackInfoReturnable<Style> cir, ChatCopy.Data data) {
		if (data == null) return;
		Style result = cir.getReturnValue();
		// RESET and removal of the last attribute can return the shared singleton.
		// Give ownership its own empty style; never tag vanilla's global EMPTY.
		if (result == Style.EMPTY) {
			result = ChatCopyStyleAccess.arctic$empty(null,
					//#if MC >= 1.21.4
					null,
					//#endif
					null, null, null, null, null, null, null, null, null);
			cir.setReturnValue(result);
		}
		((ChatCopy.Tagged) (Object) result).arctic$copyData(data);
	}

	//#else
	@Inject(method = {"copy", "flatCopy"}, at = @At("RETURN"))
	private void arctic$inherit(CallbackInfoReturnable<Style> cir) {
		((ChatCopy.Tagged) (Object) cir.getReturnValue()).arctic$copyData(arctic$copyData());
	}
	//#endif
	// Resolve at hover time so already received messages follow the current settings.
	@Inject(method = "getHoverEvent", at = @At("HEAD"), cancellable = true)
	private void arctic$hover(CallbackInfoReturnable<net.minecraft.network.chat.HoverEvent> cir) {
		ChatCopy.Data data = arctic$copyData();
		if (data == null || !data.target) return;
		if (!ChatCopy.enabled()) {
			cir.setReturnValue(null);
			return;
		}
		//#if MC >= 1.21.5
		cir.setReturnValue(new net.minecraft.network.chat.HoverEvent.ShowText(com.arcticlauncher.mod.Compat.literal("Click to copy")));
		//#else
		cir.setReturnValue(new net.minecraft.network.chat.HoverEvent(net.minecraft.network.chat.HoverEvent.Action.SHOW_TEXT,
				com.arcticlauncher.mod.Compat.literal("Click to copy")));
		//#endif
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
