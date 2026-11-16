package com.arcticlauncher.mod;

//#if MC >= 1.20.5
import net.minecraft.core.Holder;
import net.minecraft.core.component.DataComponents;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.enchantment.Enchantment;
import net.minecraft.world.item.enchantment.ItemEnchantments;
//#else
import java.util.Map;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.enchantment.Enchantment;
import net.minecraft.world.item.enchantment.EnchantmentHelper;
//#endif

/** The enchantments of a stack as one line, for the pickup feed (every version). */
final class EnchantInfo {
	static final boolean WORKS = true;

	private EnchantInfo() {}

	/** "Sharpness V, Unbreaking III", or null. Called once per new pickup row, not per frame. */
	static String summary(Object stack) {
		//#if MC >= 1.20.5
		if (!(stack instanceof ItemStack) || ((ItemStack) stack).isEmpty()) {
			return null;
		}
		ItemStack it = (ItemStack) stack;
		StringBuilder out = new StringBuilder();
		add(out, it.getEnchantments());
		add(out, it.get(DataComponents.STORED_ENCHANTMENTS));
		return out.length() == 0 ? null : out.toString();
		//#else
		// The NBT versions: the Enchantments list, or StoredEnchantments on a book.
		if (!(stack instanceof ItemStack) || ((ItemStack) stack).isEmpty()) {
			return null;
		}
		StringBuilder out = new StringBuilder();
		for (Map.Entry<Enchantment, Integer> e : EnchantmentHelper.getEnchantments((ItemStack) stack).entrySet()) {
			if (out.length() > 0) {
				out.append(", ");
			}
			out.append(e.getKey().getFullname(e.getValue()).getString());
		}
		return out.length() == 0 ? null : out.toString();
		//#endif
	}

	//#if MC >= 1.20.5
	private static void add(StringBuilder out, ItemEnchantments list) {
		if (list == null || list.isEmpty()) {
			return;
		}
		for (Holder<Enchantment> e : list.keySet()) {
			if (out.length() > 0) {
				out.append(", ");
			}
			//#if MC >= 1.21
			out.append(Enchantment.getFullname(e, list.getLevel(e)).getString());
			//#else
			// 1.20.5 and 1.20.6 look the level up by the enchantment itself.
			out.append(e.value().getFullname(list.getLevel(e.value())).getString());
			//#endif
		}
	}
	//#endif
}
