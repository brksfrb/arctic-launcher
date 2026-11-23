package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import java.text.NumberFormat;
import java.util.Locale;

public final class EnchantDescriptions {
	public static final int ITALIC = 1;
	public static final int COMPACT = 2;

	private EnchantDescriptions() {}

	public static int style(boolean book) {
		ClientConfig config = ArcticClient.config();
		if (config == null || !config.enchantDescriptions || (!book && !config.enchantDescriptionsAllItems)) {
			return -1;
		}
		Platform platform = ArcticClient.platform();
		if (config.enchantDescriptionsShiftOnly && (platform == null || !platform.shiftDown())) {
			return -1;
		}
		return config.enchantDescriptionStyle;
	}

	public static String text(String id, int level, int style, boolean oldCombat,
			boolean book, boolean armor) {
		if (style < 0 || level < 1 || id == null || !id.startsWith("minecraft:")) return null;
		String name = id.substring("minecraft:".length());
		// The legacy registry used this name through 1.20.4.
		if (name.equals("sweeping")) name = "sweeping_edge";
		if (style == COMPACT) {
			String value = compact(name, level, oldCombat, book, armor);
			if (value != null) return value;
		}
		switch (name) {
			case "aqua_affinity":
			case "bane_of_arthropods":
			case "binding_curse":
			case "blast_protection":
			case "breach":
			case "channeling":
			case "density":
			case "depth_strider":
			case "efficiency":
			case "feather_falling":
			case "fire_aspect":
			case "fire_protection":
			case "flame":
			case "fortune":
			case "frost_walker":
			case "impaling":
			case "infinity":
			case "knockback":
			case "looting":
			case "loyalty":
			case "luck_of_the_sea":
			case "lunge":
			case "lure":
			case "mending":
			case "multishot":
			case "piercing":
			case "power":
			case "projectile_protection":
			case "protection":
			case "punch":
			case "quick_charge":
			case "respiration":
			case "riptide":
			case "sharpness":
			case "silk_touch":
			case "smite":
			case "soul_speed":
			case "sweeping_edge":
			case "swift_sneak":
			case "thorns":
			case "unbreaking":
			case "vanishing_curse":
			case "wind_burst":
				return Lang.t("mods.enchant_desc." + name);
			default:
				return null;
		}
	}

	private static String compact(String name, int level, boolean oldCombat,
			boolean book, boolean armor) {
		String key = "mods.enchant_value." + name;
		switch (name) {
			// DamageEnchantment.getDamageModifier/getDamageBonus, then the vanilla enchantment JSON.
			case "sharpness":
				return Lang.t(key, number(oldCombat ? 1.25 * level : 0.5 * level + 0.5));
			case "smite":
			case "bane_of_arthropods":
				return Lang.t(key, number(2.5 * level));
			// ProtectionEnchantment and CombatRules.getDamageAfterMagicAbsorb. In 1.8.9
			// the effective points are randomized, so show the contribution before that roll.
			case "protection":
			case "fire_protection":
			case "feather_falling":
			case "blast_protection":
			case "projectile_protection":
				if (oldCombat) {
					double factor = name.equals("protection") ? 0.75 : name.equals("fire_protection") ? 1.25
							: name.equals("feather_falling") ? 2.5 : 1.5;
					return Lang.t("mods.enchant_value.protection_legacy",
							number(Math.floor((6.0 + (double) level * level) / 3.0 * factor)));
				}
				int points = name.equals("protection") ? 1 : name.equals("feather_falling") ? 3 : 2;
				return Lang.t(key, number(4 * Math.min(20L, (long) points * level)));
			// UnbreakingEnchantment.shouldPreventDamage first rolls a 60% bypass on armor.
			// The JSON-era equivalent uses distinct equipment predicates and binomial chances.
			case "unbreaking":
				double chance = 100.0 * level / (level + 1.0);
				if (book) return Lang.t(key + "_book", number(chance), number(chance * 0.4));
				if (armor) return Lang.t(key + "_armor", number(chance * 0.4));
				return Lang.t(key, number(chance));
			case "mending":
				return Lang.t(key);
			case "piercing":
				return Lang.t(key, number(level + 1.0));
			default:
				return null;
		}
	}

	private static String number(double value) {
		NumberFormat format = NumberFormat.getNumberInstance(Locale.forLanguageTag(Lang.code()));
		format.setMaximumFractionDigits(2);
		return format.format(value);
	}
}
