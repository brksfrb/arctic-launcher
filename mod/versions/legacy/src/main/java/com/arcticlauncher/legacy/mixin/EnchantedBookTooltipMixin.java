package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.EnchantTooltip;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Share;
import com.llamalad7.mixinextras.sugar.ref.LocalRef;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Slice;
import java.util.List;
import net.minecraft.enchantment.Enchantment;
import net.minecraft.item.EnchantedBookItem;

@Mixin(EnchantedBookItem.class)
abstract class EnchantedBookTooltipMixin {
	private static final String NAME = "Lnet/minecraft/enchantment/Enchantment;getTranslatedName(I)Ljava/lang/String;";
	//#if MC >= 1.12
	private static final String BOOK_TOOLTIP = "appendTooltips";
	//#else
	private static final String BOOK_TOOLTIP = "appendTooltip";
	//#endif

	@WrapOperation(method = BOOK_TOOLTIP, at = @At(value = "INVOKE", target = NAME))
	private String arctic$describe(Enchantment enchantment, int level, Operation<String> original,
			@Share("description") LocalRef<String> description) {
		description.set(EnchantTooltip.describe(enchantment, level, true, false));
		return original.call(enchantment, level);
	}

	@WrapOperation(method = BOOK_TOOLTIP, slice = @Slice(from = @At(value = "INVOKE", target = NAME)),
			at = @At(value = "INVOKE", target = "Ljava/util/List;add(Ljava/lang/Object;)Z", ordinal = 0))
	private boolean arctic$append(List<String> lines, Object name, Operation<Boolean> original,
			@Share("description") LocalRef<String> description) {
		boolean added = original.call(lines, name);
		if (added && description.get() != null) lines.add(description.get());
		return added;
	}
}
