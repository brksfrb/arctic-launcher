package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyScroll;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.gui.widget.ListWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.injection.At;

/** Apply {@link LegacyScroll} so vanilla lists move a full row per notch. */
@Mixin(ListWidget.class)
abstract class ListWheelLegacyMixin {
	@Shadow @Final protected int entryHeight;
	@Shadow protected float scrollAmount;

	@WrapOperation(method = "handleMouse", at = @At(value = "INVOKE", target = "Lorg/lwjgl/input/Mouse;getEventDWheel()I"))
	private int arctic$wheel(Operation<Integer> original) {
		// Vanilla moves half an entry per sign-clamped notch. Use the shared
		// amount (two modern units per notch) so a notch moves one full row,
		// and return 0 so vanilla does not apply its own delta as well.
		scrollAmount -= (float) (LegacyScroll.amount(original.call()) * entryHeight / 2.0);
		return 0;
	}
}
