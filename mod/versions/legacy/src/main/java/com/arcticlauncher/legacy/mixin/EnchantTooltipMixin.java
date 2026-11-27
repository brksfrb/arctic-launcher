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
import net.minecraft.item.ItemStack;
import net.minecraft.item.ArmorItem;

@Mixin(ItemStack.class)
abstract class EnchantTooltipMixin {
	private static final String NAME = "Lnet/minecraft/enchantment/Enchantment;getTranslatedName(I)Ljava/lang/String;";

	@WrapOperation(method = "getTooltip", at = @At(value = "INVOKE", target = NAME))
	private String arctic$describe(Enchantment enchantment, int level, Operation<String> original,
			@Share("description") LocalRef<String> description) {
		description.set(EnchantTooltip.describe(enchantment, level, false, ((ItemStack) (Object) this).getItem() instanceof ArmorItem));
		return original.call(enchantment, level);
	}

	@WrapOperation(method = "getTooltip", slice = @Slice(from = @At(value = "INVOKE", target = NAME)),
			at = @At(value = "INVOKE", target = "Ljava/util/List;add(Ljava/lang/Object;)Z", ordinal = 0))
	private boolean arctic$append(List<String> lines, Object name, Operation<Boolean> original,
			@Share("description") LocalRef<String> description) {
		boolean added = original.call(lines, name);
		if (added && description.get() != null) lines.add(description.get());
		return added;
	}
}
