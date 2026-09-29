package com.arcticlauncher.mod;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.client.Minecraft;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.ItemStack;

/**
 * What the game HUD widgets read (armor, effects, held item, target).
 * Wired up where the game features are (see {@link Compat#FEATURES}).
 */
public final class GameInfo {
	private static final int TICKS_PER_SECOND = 20;
	// Long enough to bridge a missed aim mid-fight, short enough to go away
	// right after (it lingered 5 s).
	private static final long TARGET_MEMORY_MS = 1500;
	private static final String[] ROMAN = {"", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"};

	private GameInfo() {}

	/** Preview stacks by item id, made once. */
	private static final Map<String, Object> SAMPLES = new HashMap<>();
	private static Object samplesLevel;

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
		if (p == null) {
			return -1;
		}
		// From the eyes to the nearest point of the target's box.
		net.minecraft.world.phys.AABB box = target.getBoundingBox();
		net.minecraft.world.phys.Vec3 eye = p.getEyePosition(1f);
		double dx = Math.max(0, Math.max(box.minX - eye.x, eye.x - box.maxX));
		double dy = Math.max(0, Math.max(box.minY - eye.y, eye.y - box.maxY));
		double dz = Math.max(0, Math.max(box.minZ - eye.z, eye.z - box.maxZ));
		return Math.sqrt(dx * dx + dy * dy + dz * dz);
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

	/** {name, left, max, slot} for worn and held items that wear out. */
	public static List<Object[]> durability() {
		LocalPlayer p = player();
		if (p == null) {
			return Collections.emptyList();
		}
		List<Object[]> rows = new ArrayList<>();
		for (EquipmentSlot slot : new EquipmentSlot[] {EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET,
				EquipmentSlot.MAINHAND, EquipmentSlot.OFFHAND}) {
			ItemStack stack = p.getItemBySlot(slot);
			if (!stack.isEmpty() && stack.isDamageableItem()) {
				rows.add(new Object[] {stack.getHoverName().getString(), stack.getMaxDamage() - stack.getDamageValue(),
						stack.getMaxDamage(), slot.getName()});
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
			//#if MC >= 1.20.6
			MobEffect effect = e.getEffect().value();
			//#else
			MobEffect effect = e.getEffect();
			//#endif
			int level = e.getAmplifier();
			String name = effect.getDisplayName().getString() + (level > 0 && level < ROMAN.length ? " " + ROMAN[level] : "");
			rows.add(new Object[] {name, time(e), effect.getColor(), sprite(e)});
		}
		return rows;
	}

	/** The mob effect's icon: an id into the GUI sprite atlas (1.21.6+) or a raw atlas sprite before that. */
	private static Object sprite(MobEffectInstance e) {
		//#if MC >= 1.21.6
		return Compat.mobEffectSprite(e.getEffect());
		//#elif MC >= 1.20.6
		return Minecraft.getInstance().getMobEffectTextures().get(e.getEffect());
		//#else
		return Minecraft.getInstance().getMobEffectTextures().get(e.getEffect());
		//#endif
	}

	private static String time(MobEffectInstance e) {
		//#if MC >= 1.19.4
		if (e.isInfiniteDuration()) {
			return "∞";
		}
		//#endif
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
		Inventory inventory = Compat.inventory(p);
		int total = 0;
		for (int i = 0; i < inventory.getContainerSize(); i++) {
			ItemStack it = inventory.getItem(i);
			if (it.getItem() == held.getItem()) {
				total += it.getCount();
			}
		}
		return new Object[] {held, total};
	}

	/** Null outside a world: item data isn't bound to the registries yet. */
	public static Object sampleItem(String id) {
		Object level = Minecraft.getInstance().level;
		if (level == null) {
			return null;
		}
		if (level != samplesLevel) {
			// A new world may bring new item data; don't reuse old stacks.
			SAMPLES.clear();
			samplesLevel = level;
		}
		return SAMPLES.computeIfAbsent(id, k -> BuiltInRegistries.ITEM.getOptional(Identifier.tryParse(k)).map(ItemStack::new).orElse(null));
	}

	public static Object effectSprite(String id) {
		return BuiltInRegistries.MOB_EFFECT.getOptional(Identifier.tryParse(id)).map(GameInfo::spriteFor).orElse(null);
	}

	private static Object spriteFor(MobEffect effect) {
		//#if MC >= 1.21.6
		return Compat.mobEffectSprite(BuiltInRegistries.MOB_EFFECT.wrapAsHolder(effect));
		//#elif MC >= 1.20.6
		return Minecraft.getInstance().getMobEffectTextures().get(BuiltInRegistries.MOB_EFFECT.wrapAsHolder(effect));
		//#else
		return Minecraft.getInstance().getMobEffectTextures().get(effect);
		//#endif
	}

	/** What you aim at, or what you hit in the last few seconds. */
	public static Object[] target() {
		Entity aimed = Minecraft.getInstance().crosshairPickEntity;
		Entity e = aimed instanceof LivingEntity ? aimed
				: System.currentTimeMillis() - lastAttackTime < TARGET_MEMORY_MS ? lastAttacked : null;
		if (!(e instanceof LivingEntity) || !((LivingEntity) e).isAlive()) {
			return null;
		}
		LivingEntity living = (LivingEntity) e;
		return new Object[] {living.getName().getString(), living.getHealth(), living.getMaxHealth()};
	}
}
