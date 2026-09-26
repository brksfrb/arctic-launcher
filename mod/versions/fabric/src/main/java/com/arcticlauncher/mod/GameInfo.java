package com.arcticlauncher.mod;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
//#if MC >= 26.3
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.ItemStack;
//#endif

/**
 * What the game HUD widgets read (armor, effects, held item, target).
 * Wired up where the game features are (see {@link Compat#FEATURES}).
 */
public final class GameInfo {
	private static final int TICKS_PER_SECOND = 20;
	private static final long TARGET_MEMORY_MS = 5000;
	private static final String[] ROMAN = {"", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"};

	private GameInfo() {}

	//#if MC >= 26.3
	private static Entity lastAttacked;
	private static long lastAttackTime;

	private static LocalPlayer player() {
		return Minecraft.getInstance().player;
	}

	/** You hit {@code target}: remember it and report the reach. */
	public static double attacked(Entity target) {
		lastAttacked = target;
		lastAttackTime = System.currentTimeMillis();
		LocalPlayer p = player();
		return p == null ? -1 : Math.sqrt(target.getBoundingBox().distanceToSqr(p.getEyePosition()));
	}

	public static int hurtTime() {
		LocalPlayer p = player();
		return p == null ? 0 : p.hurtTime;
	}

	public static List<Object[]> armor() {
		LocalPlayer p = player();
		if (p == null) {
			return Collections.emptyList();
		}
		List<Object[]> rows = new ArrayList<>();
		for (EquipmentSlot slot : new EquipmentSlot[] {EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET, EquipmentSlot.MAINHAND}) {
			ItemStack stack = p.getItemBySlot(slot);
			if (!stack.isEmpty()) {
				String text = stack.isDamageableItem() ? String.valueOf(stack.getMaxDamage() - stack.getDamageValue()) : null;
				rows.add(new Object[] {stack, text});
			}
		}
		return rows;
	}

	public static List<Object[]> effects() {
		LocalPlayer p = player();
		if (p == null) {
			return Collections.emptyList();
		}
		List<Object[]> rows = new ArrayList<>();
		for (MobEffectInstance e : p.getActiveEffects()) {
			MobEffect effect = e.getEffect().value();
			int level = e.getAmplifier();
			String name = effect.getDisplayName().getString() + (level > 0 && level < ROMAN.length ? " " + ROMAN[level] : "");
			rows.add(new Object[] {name, time(e), effect.getColor(), net.minecraft.client.gui.Hud.getMobEffectSprite(e.getEffect())});
		}
		return rows;
	}

	private static String time(MobEffectInstance e) {
		if (e.isInfiniteDuration()) {
			return "∞";
		}
		int seconds = e.getDuration() / TICKS_PER_SECOND;
		return seconds / 60 + ":" + String.format("%02d", seconds % 60);
	}

	public static Object[] heldItem() {
		LocalPlayer p = player();
		if (p == null) {
			return null;
		}
		ItemStack held = p.getMainHandItem();
		if (held.isEmpty()) {
			return null;
		}
		Inventory inventory = p.getInventory();
		int total = 0;
		for (int i = 0; i < inventory.getContainerSize(); i++) {
			ItemStack it = inventory.getItem(i);
			if (ItemStack.isSameItem(it, held)) {
				total += it.getCount();
			}
		}
		return new Object[] {held, total};
	}

	/** What you aim at, or what you hit in the last few seconds. */
	public static Object[] target() {
		Entity aimed = Minecraft.getInstance().crosshairPickEntity;
		Entity e = aimed instanceof LivingEntity ? aimed
				: System.currentTimeMillis() - lastAttackTime < TARGET_MEMORY_MS ? lastAttacked : null;
		if (!(e instanceof LivingEntity living) || !living.isAlive()) {
			return null;
		}
		return new Object[] {living.getName().getString(), living.getHealth(), living.getMaxHealth()};
	}
	//#else
	public static int hurtTime() {
		return 0;
	}

	public static List<Object[]> armor() {
		return Collections.emptyList();
	}

	public static List<Object[]> effects() {
		return Collections.emptyList();
	}

	public static Object[] heldItem() {
		return null;
	}

	public static Object[] target() {
		return null;
	}
	//#endif
}
