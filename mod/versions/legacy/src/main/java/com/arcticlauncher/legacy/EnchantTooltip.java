package com.arcticlauncher.legacy;

import com.arcticlauncher.client.feature.EnchantDescriptions;
import net.minecraft.enchantment.Enchantment;
import net.minecraft.util.Identifier;

public final class EnchantTooltip {
	private EnchantTooltip() {}

	public static String describe(Enchantment enchantment, int level, boolean book, boolean armor) {
		int style = EnchantDescriptions.style(book);
		if (style < 0) return null;
		//#if MC >= 1.9
		Identifier id = Enchantment.REGISTRY.getIdentifier(enchantment);
		boolean oldCombat = false;
		//#else
		Identifier id = null;
		for (Identifier candidate : Enchantment.getSet()) {
			if (candidate.getNamespace().equals("minecraft") && Enchantment.getByName(candidate.toString()) == enchantment) {
				id = candidate;
				break;
			}
		}
		boolean oldCombat = true;
		//#endif
		String line = EnchantDescriptions.text(id == null ? null : id.toString(), level, style, oldCombat,
				book, armor);
		if (line == null) return null;
		if (style == EnchantDescriptions.ITALIC) return "§7§o ▸ " + line;
		return (style == EnchantDescriptions.COMPACT ? "§b" : "§8") + line;
	}
}
