//#if MC >= 1.20.5
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.EnchantTooltip;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Share;
import com.llamalad7.mixinextras.sugar.ref.LocalRef;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Slice;
import java.util.function.Consumer;
//#if MC >= 1.21
import net.minecraft.core.Holder;
//#else
import net.minecraft.core.registries.BuiltInRegistries;
//#endif
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.enchantment.ItemEnchantments;
import net.minecraft.world.item.enchantment.Enchantment;

@Mixin(ItemEnchantments.class)
abstract class EnchantTooltipComponentsMixin {
	private static final String TOOLTIP = "addToTooltip";

	//#if MC >= 1.21
	@WrapOperation(method = TOOLTIP, at = @At(value = "INVOKE", target = "Lnet/minecraft/world/item/enchantment/Enchantment;getFullname(Lnet/minecraft/core/Holder;I)Lnet/minecraft/network/chat/Component;"))
	private Component arctic$describe(Holder<Enchantment> enchantment, int level, Operation<Component> original,
			@Share("description") LocalRef<Component> description) {
		String id = enchantment.unwrapKey().map(key ->
				//#if MC >= 1.21.11
				key.identifier().toString()
				//#else
				key.location().toString()
				//#endif
		).orElse(null);
		description.set(EnchantTooltip.describe(id, level));
		return original.call(enchantment, level);
	}
	//#else
	@WrapOperation(method = TOOLTIP, at = @At(value = "INVOKE", target = "Lnet/minecraft/world/item/enchantment/Enchantment;getFullname(I)Lnet/minecraft/network/chat/Component;"))
	private Component arctic$describe(Enchantment enchantment, int level, Operation<Component> original,
			@Share("description") LocalRef<Component> description) {
		description.set(EnchantTooltip.describe(String.valueOf(BuiltInRegistries.ENCHANTMENT.getKey(enchantment)), level));
		return original.call(enchantment, level);
	}
	//#endif

	@WrapOperation(method = TOOLTIP, at = @At(value = "INVOKE", target = "Ljava/util/function/Consumer;accept(Ljava/lang/Object;)V"))
	private void arctic$append(Consumer<Component> lines, Object name, Operation<Void> original,
			@Share("description") LocalRef<Component> description) {
		original.call(lines, name);
		Component line = description.get();
		description.set(null);
		if (line != null) lines.accept(line);
	}
}
//#endif
