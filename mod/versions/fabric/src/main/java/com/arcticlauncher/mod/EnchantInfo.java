package com.arcticlauncher.mod;

//#if MC >= 1.21
import net.minecraft.core.Holder;
import net.minecraft.core.component.DataComponents;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.enchantment.Enchantment;
import net.minecraft.world.item.enchantment.ItemEnchantments;
//#endif

/** The enchantments of a stack as one line, for the pickup feed (1.21 and newer; older games show none). */
final class EnchantInfo {
	//#if MC >= 1.21
	static final boolean WORKS = true;
	//#else
	static final boolean WORKS = false;
	//#endif

	private EnchantInfo() {}

	/** "Sharpness V, Unbreaking III", or null. Called once per new pickup row, not per frame. */
	static String summary(Object stack) {
		//#if MC >= 1.21
		if (!(stack instanceof ItemStack) || ((ItemStack) stack).isEmpty()) {
			return null;
		}
		ItemStack it = (ItemStack) stack;
		StringBuilder out = new StringBuilder();
		add(out, it.getEnchantments());
		add(out, it.get(DataComponents.STORED_ENCHANTMENTS));
		return out.length() == 0 ? null : out.toString();
		//#else
		return null;
		//#endif
	}

	//#if MC >= 1.21
	private static void add(StringBuilder out, ItemEnchantments list) {
		if (list == null || list.isEmpty()) {
			return;
		}
		for (Holder<Enchantment> e : list.keySet()) {
			if (out.length() > 0) {
				out.append(", ");
			}
			out.append(Enchantment.getFullname(e, list.getLevel(e)).getString());
		}
	}
	//#endif
}
