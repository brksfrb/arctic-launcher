//#if MC < 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.EnchantTooltip;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Share;
import com.llamalad7.mixinextras.sugar.ref.LocalRef;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Slice;
import java.util.List;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.EnchantedBookItem;
import net.minecraft.world.item.enchantment.Enchantment;

@Mixin(EnchantedBookItem.class)
abstract class EnchantTooltipBookMixin {
	private static final String TOOLTIP = "appendHoverText";

	@WrapOperation(method = TOOLTIP, at = @At(value = "INVOKE", target = "Lnet/minecraft/world/item/enchantment/Enchantment;getFullname(I)Lnet/minecraft/network/chat/Component;"))
	private Component arctic$describe(Enchantment enchantment, int level, Operation<Component> original,
			@Share("description") LocalRef<Component> description) {
		description.set(EnchantTooltip.describe(String.valueOf(BuiltInRegistries.ENCHANTMENT.getKey(enchantment)), level));
		return original.call(enchantment, level);
	}

	@WrapOperation(method = TOOLTIP,
			slice = @Slice(from = @At(value = "INVOKE", target = "Lnet/minecraft/world/item/enchantment/Enchantment;getFullname(I)Lnet/minecraft/network/chat/Component;")),
			at = @At(value = "INVOKE", target = "Ljava/util/List;add(Ljava/lang/Object;)Z", ordinal = 0))
	private boolean arctic$append(List<Component> lines, Object name, Operation<Boolean> original,
			@Share("description") LocalRef<Component> description) {
		boolean added = original.call(lines, name);
		if (added && description.get() != null) lines.add(description.get());
		return added;
	}
}
//#endif
