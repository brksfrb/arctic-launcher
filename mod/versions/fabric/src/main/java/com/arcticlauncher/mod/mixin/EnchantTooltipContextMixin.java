package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.EnchantTooltip;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import java.util.List;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.TooltipFlag;
//#if MC >= 1.20.5
import net.minecraft.world.item.Item;
//#endif
import org.spongepowered.asm.mixin.Mixin;

@Mixin(ItemStack.class)
abstract class EnchantTooltipContextMixin {
	@WrapMethod(method = "getTooltipLines")
	//#if MC >= 1.20.5
	private List<Component> arctic$tooltip(Item.TooltipContext context, Player player, TooltipFlag flags,
			Operation<List<Component>> original) {
		Object[] args = {context, player, flags};
	//#else
	private List<Component> arctic$tooltip(Player player, TooltipFlag flags, Operation<List<Component>> original) {
		Object[] args = {player, flags};
	//#endif
		if (ArcticClient.config() == null || !ArcticClient.config().enchantDescriptions) {
			return original.call(args);
		}
		ItemStack previous = EnchantTooltip.enter((ItemStack) (Object) this);
		try {
			return original.call(args);
		} finally {
			EnchantTooltip.leave(previous);
		}
	}
}
