package com.arcticlauncher.mod;

import com.arcticlauncher.client.feature.EnchantDescriptions;
import net.minecraft.ChatFormatting;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
//#if MC >= 1.21
import net.minecraft.tags.ItemTags;
//#else
import net.minecraft.world.item.ArmorItem;
//#endif

/** Context for the vanilla tooltip helpers, which receive enchantments without their owning stack. */
public final class EnchantTooltip {
	private static final ThreadLocal<ItemStack> STACK = new ThreadLocal<>();

	private EnchantTooltip() {}

	public static ItemStack enter(ItemStack stack) {
		ItemStack previous = STACK.get();
		STACK.set(stack);
		return previous;
	}

	public static void leave(ItemStack previous) {
		if (previous == null) STACK.remove();
		else STACK.set(previous);
	}

	public static Component describe(String id, int level) {
		ItemStack stack = STACK.get();
		if (stack == null) return null;
		boolean book = stack.getItem() == Items.ENCHANTED_BOOK;
		int style = EnchantDescriptions.style(book);
		if (style < 0) return null;
		//#if MC >= 1.21
		boolean armor = stack.is(ItemTags.ARMOR_ENCHANTABLE);
		//#else
		boolean armor = stack.getItem() instanceof ArmorItem;
		//#endif
		String line = EnchantDescriptions.text(id, level, style, false, book, armor);
		if (line == null) return null;
		if (style == EnchantDescriptions.ITALIC) {
			return literal(" ▸ " + line).withStyle(ChatFormatting.GRAY, ChatFormatting.ITALIC);
		}
		return literal(line).withStyle(style == EnchantDescriptions.COMPACT ? ChatFormatting.AQUA : ChatFormatting.DARK_GRAY);
	}

	private static net.minecraft.network.chat.MutableComponent literal(String text) {
		//#if MC >= 1.19
		return Component.literal(text);
		//#else
		return new net.minecraft.network.chat.TextComponent(text);
		//#endif
	}
}
