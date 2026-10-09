package com.arcticlauncher.legacy;

import java.awt.image.BufferedImage;
import java.io.File;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.List;
import java.util.UUID;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.GameKey;
import com.arcticlauncher.client.MenuAction;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.looks.Looks;
import com.arcticlauncher.client.ui.Page;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.ConnectScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.SettingsScreen;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.gui.screen.multiplayer.MultiplayerScreen;
import net.minecraft.client.gui.screen.options.LanguageOptionsScreen;
import net.minecraft.client.gui.screen.world.SelectWorldScreen;
import net.minecraft.client.network.PlayerListEntry;
import net.minecraft.client.network.ServerInfo;
import net.minecraft.client.option.GameOptions;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.resource.ResourcePackLoader;
import net.minecraft.client.sound.PositionedSoundInstance;
import net.minecraft.entity.Entity;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.effect.StatusEffect;
import net.minecraft.entity.effect.StatusEffectInstance;
import net.minecraft.entity.mob.Monster;
import net.minecraft.entity.player.ClientPlayerEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import net.minecraft.util.Identifier;
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.math.BlockPos;
import net.minecraft.world.LightType;

/** {@link Platform} for old Minecraft (1.8.9) on Legacy Fabric. */
public final class LegacyPlatform implements Platform {
	private static final int TICKS_PER_SECOND = 20;
	private static final String[] ROMAN = {"", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"};
	// Long enough to bridge a missed aim mid-fight, short enough to go away
	// right after (it lingered 5 s).
	private static final long TARGET_MEMORY_MS = 1500;
	private static final float FULLBRIGHT_GAMMA = 16f;
	private static final float MENTION_PITCH = 1.6f;

	/** Perspective before Freelook switched to third person (-1: none). */
	private int perspectiveBefore = -1;
	/** Gamma before Fullbright took over (NaN: not overriding). */
	private float gammaBefore = Float.NaN;
	private static Entity lastAttacked;
	private static long lastAttackTime;

	private static MinecraftClient mc() {
		return MinecraftClient.getInstance();
	}

	// ---- Basics --------------------------------------------------------------------

	@Override
	public String minecraftVersion() {
		return FabricLoader.getInstance().getModContainer("minecraft")
				.map(c -> c.getMetadata().getVersion().getFriendlyString()).orElse("1.8.9");
	}

	@Override
	public File configDir() {
		return FabricLoader.getInstance().getConfigDir().toFile();
	}

	@Override
	public void log(boolean warning, String message) {
		if (warning) {
			ArcticLegacy.LOG.warn(message);
		} else {
			ArcticLegacy.LOG.info(message);
		}
	}

	@Override
	public int fps() {
		return MinecraftClient.getCurrentFps();
	}

	@Override
	public int ping() {
		MinecraftClient mc = mc();
		if (mc.getNetworkHandler() == null || mc.player == null || mc.isInSingleplayer()) {
			return -1;
		}
		PlayerListEntry entry = mc.getNetworkHandler().getPlayerListEntry(mc.player.getUuid());
		return entry == null ? -1 : entry.getLatency();
	}

	@Override
	public boolean inWorld() {
		return mc().world != null && mc().player != null;
	}

	@Override
	public double[] position() {
		ClientPlayerEntity p = mc().player;
		return p == null ? null : new double[] {p.x, p.y, p.z, p.yaw, p.pitch};
	}

	@Override
	public boolean keyDown(GameKey key) {
		return binding(key).isPressed();
	}

	private static KeyBinding binding(GameKey key) {
		GameOptions o = mc().options;
		switch (key) {
			case FORWARD:
				return o.forwardKey;
			case BACK:
				return o.backKey;
			case LEFT:
				return o.leftKey;
			case RIGHT:
				return o.rightKey;
			case JUMP:
				return o.jumpKey;
			case SNEAK:
				return o.sneakKey;
			case SPRINT:
				return o.sprintKey;
			case ATTACK:
				return o.attackKey;
			default:
				return o.useKey;
		}
	}

	@Override
	public boolean physicalKeyDown(GameKey key) {
		return LegacyKeys.isDown(binding(key).getCode());
	}

	@Override
	public void setKeyDown(GameKey key, boolean down) {
		KeyBinding.setKeyPressed(binding(key).getCode(), down);
	}

	@Override
	public String biome() {
		MinecraftClient mc = mc();
		if (mc.world == null || mc.player == null) {
			return null;
		}
		//#if MC >= 1.9
		return mc.world.getBiome(mc.player.getBlockPos()).getName();
		//#else
		return mc.world.getBiome(mc.player.getBlockPos()).name;
		//#endif
	}

	@Override
	public String server() {
		MinecraftClient mc = mc();
		if (mc.world == null) {
			return null;
		}
		if (mc.isInSingleplayer() || mc.getCurrentServerEntry() == null) {
			return "Singleplayer";
		}
		return mc.getCurrentServerEntry().address;
	}

	@Override
	public long dayTime() {
		return mc().world == null ? -1 : mc().world.getTimeOfDay();
	}

	@Override
	public boolean isKeyDown(String key) {
		return LegacyKeys.isDown(key);
	}

	@Override
	public String keyLabel(String key) {
		return LegacyKeys.label(key);
	}

	@Override
	public String keyName(int nativeKey) {
		return LegacyKeys.name(nativeKey);
	}

	@Override
	public void setThirdPerson(boolean on) {
		GameOptions options = mc().options;
		if (on) {
			perspectiveBefore = options.perspective;
			options.perspective = 1;
		} else if (perspectiveBefore >= 0) {
			options.perspective = perspectiveBefore;
			perspectiveBefore = -1;
		}
	}

	@Override
	public boolean hasFeatures() {
		return true;
	}

	/** How long Minecraft shows an achievement pop-up (ms); pop-ups and 1.12's toasts are this tall. */
	private static final long ACHIEVEMENT_MS = 3000;
	private static final int ACHIEVEMENT_H = 32;

	//#if MC >= 1.12
	/** The toast manager's on-screen slots (a package-private type, so read reflectively). */
	private static java.lang.reflect.Field toastSlotsField;
	private static boolean toastSlotsTried;

	private static Object[] toastSlots(Object toasts) {
		if (!toastSlotsTried) {
			toastSlotsTried = true;
			try {
				toastSlotsField = toasts.getClass().getDeclaredField("field_15921");
				toastSlotsField.setAccessible(true);
			} catch (ReflectiveOperationException e) {
				ArcticLegacy.LOG.warn("Arctic: can't see the toasts ({}); notices may overlap them", e.toString());
			}
		}
		try {
			return toastSlotsField == null ? new Object[0] : (Object[]) toastSlotsField.get(toasts);
		} catch (ReflectiveOperationException e) {
			return new Object[0];
		}
	}
	//#endif

	@Override
	public int toastsBottom() {
		//#if MC >= 1.12
		Object[] slots = toastSlots(((com.arcticlauncher.legacy.mixin.ToastsAccess) mc()).arctic$toasts());
		for (int i = slots.length - 1; i >= 0; i--) {
			if (slots[i] != null) {
				return (i + 1) * ACHIEVEMENT_H;
			}
		}
		return 0;
		//#else
		com.arcticlauncher.legacy.mixin.AchievementNotificationAccess n =
				(com.arcticlauncher.legacy.mixin.AchievementNotificationAccess) mc().notification;
		if (n == null || n.arctic$achievement() == null) {
			return 0;
		}
		boolean showing = n.arctic$permanent() || MinecraftClient.getTime() - n.arctic$time() < ACHIEVEMENT_MS;
		return showing ? ACHIEVEMENT_H : 0;
		//#endif
	}

	@Override
	public boolean outlineTweaks() {
		return true;
	}

	@Override
	public boolean hitColorWorks() {
		return true;
	}

	@Override
	public boolean motionBlurWorks() {
		return true;
	}

	@Override
	public boolean minimapWorks() {
		return true;
	}

	@Override
	public String minimap() {
		return LegacyMinimap.refresh() ? "dyn:" + LegacyMinimap.KEY : null;
	}

	@Override
	public boolean itemPhysicsWorks() {
		return true;
	}

	@Override
	public boolean scoreboardTweaks() {
		return true;
	}

	@Override
	public boolean smoothFont() {
		return LegacySmoothFont.on();
	}

	/** Nothing to reload: the text renderer asks LegacySmoothFont every time it draws. */
	@Override
	public void setSmoothFont(boolean on) {}

	/** Each tick: Fullbright raises gamma while on, and gives it back after. */
	void tick() {
		GameOptions options = mc().options;
		boolean bright = ArcticClient.features() != null && ArcticClient.features().fullbright();
		if (bright) {
			if (Float.isNaN(gammaBefore)) {
				gammaBefore = options.gamma;
			}
			options.gamma = FULLBRIGHT_GAMMA;
		} else if (!Float.isNaN(gammaBefore)) {
			options.gamma = gammaBefore;
			gammaBefore = Float.NaN;
		}
	}

	// ---- Player and world ----------------------------------------------------------

	@Override
	public int hurtTime() {
		ClientPlayerEntity p = mc().player;
		return p == null ? 0 : p.hurtTime;
	}

	@Override
	public List<Object[]> armor() {
		ClientPlayerEntity p = mc().player;
		if (p == null) {
			return Collections.emptyList();
		}
		List<Object[]> rows = new ArrayList<Object[]>();
		// Helmet first (the list is boots → helmet), then the held item.
		List<ItemStack> armor = armorSlots(p);
		for (int i = armor.size() - 1; i >= 0; i--) {
			addArmorRow(rows, armor.get(i));
		}
		addArmorRow(rows, p.inventory.getMainHandStack());
		return rows;
	}

	/** Worn armor, boots first. */
	private static List<ItemStack> armorSlots(ClientPlayerEntity p) {
		//#if MC >= 1.11
		return p.inventory.field_15083;
		//#else
		return java.util.Arrays.asList(p.inventory.armor);
		//#endif
	}

	/** The 36 main inventory slots (hotbar first). */
	private static List<ItemStack> mainSlots(ClientPlayerEntity p) {
		//#if MC >= 1.11
		return p.inventory.field_15082;
		//#else
		return java.util.Arrays.asList(p.inventory.main);
		//#endif
	}

	/** Something in the slot (1.11+ uses an empty stack for nothing, older versions null). */
	private static boolean present(ItemStack stack) {
		//#if MC >= 1.11
		return stack != null && !stack.isEmpty();
		//#else
		return stack != null;
		//#endif
	}

	private static int count(ItemStack stack) {
		//#if MC >= 1.11
		return stack.getCount();
		//#else
		return stack.count;
		//#endif
	}

	private static void addArmorRow(List<Object[]> rows, ItemStack stack) {
		if (present(stack)) {
			String text = stack.isDamageable() ? String.valueOf(stack.getMaxDamage() - stack.getDamage()) : null;
			rows.add(new Object[] {stack, text});
		}
	}

	@Override
	public List<Object[]> effects() {
		ClientPlayerEntity p = mc().player;
		if (p == null) {
			return Collections.emptyList();
		}
		List<Object[]> rows = new ArrayList<Object[]>();
		Collection<StatusEffectInstance> active = p.getStatusEffectInstances();
		for (StatusEffectInstance e : active) {
			//#if MC >= 1.9
			StatusEffect effect = e.getStatusEffect();
			//#else
			StatusEffect effect = StatusEffect.STATUS_EFFECTS[e.getEffectId()];
			//#endif
			if (effect == null) {
				continue;
			}
			int level = e.getAmplifier();
			String name = net.minecraft.client.resource.language.I18n.translate(effect.getTranslationKey())
					+ (level > 0 && level < ROMAN.length ? " " + ROMAN[level] : "");
			int seconds = e.getDuration() / TICKS_PER_SECOND;
			String time = seconds / 60 + ":" + String.format("%02d", seconds % 60);
			Object sprite = effect.hasIcon() ? Integer.valueOf(effect.getIconLevel()) : null;
			rows.add(new Object[] {name, time, effect.getColor(), sprite});
		}
		return rows;
	}

	@Override
	public Object[] heldItem() {
		ClientPlayerEntity p = mc().player;
		if (p == null || !present(p.inventory.getMainHandStack())) {
			return null;
		}
		ItemStack held = p.inventory.getMainHandStack();
		int total = 0;
		for (ItemStack it : mainSlots(p)) {
			if (present(it) && it.getItem() == held.getItem() && it.getDamage() == held.getDamage()) {
				total += count(it);
			}
		}
		return new Object[] {held, total};
	}

	@Override
	public Object sampleItem(String id) {
		String path = id.contains(":") ? id.substring(id.indexOf(':') + 1) : id;
		net.minecraft.item.Item item = net.minecraft.item.Item.REGISTRY.get(new Identifier(path));
		return item == null ? null : new ItemStack(item);
	}

	@Override
	public Object effectSprite(String id) {
		String path = id.contains(":") ? id.substring(id.indexOf(':') + 1) : id;
		StatusEffect effect = StatusEffect.get(path);
		return effect != null && effect.hasIcon() ? Integer.valueOf(effect.getIconLevel()) : null;
	}

	/** You hit something (from the attack hook): remember it and report the reach. */
	static double attacked(Entity target) {
		lastAttacked = target;
		lastAttackTime = System.currentTimeMillis();
		ClientPlayerEntity p = mc().player;
		if (p == null) {
			return -1;
		}
		double eyeY = p.y + p.getEyeHeight();
		double dx = Math.max(target.getBoundingBox().minX - p.x, Math.max(0, p.x - target.getBoundingBox().maxX));
		double dy = Math.max(target.getBoundingBox().minY - eyeY, Math.max(0, eyeY - target.getBoundingBox().maxY));
		double dz = Math.max(target.getBoundingBox().minZ - p.z, Math.max(0, p.z - target.getBoundingBox().maxZ));
		return Math.sqrt(dx * dx + dy * dy + dz * dz);
	}

	@Override
	public Object[] target() {
		Entity aimed = aimedEntity();
		Entity e = aimed instanceof LivingEntity ? aimed
				: System.currentTimeMillis() - lastAttackTime < TARGET_MEMORY_MS ? lastAttacked : null;
		if (!(e instanceof LivingEntity) || !e.isAlive()) {
			return null;
		}
		LivingEntity living = (LivingEntity) e;
		return new Object[] {living.getName().asUnformattedString(), living.getHealth(), living.getMaxHealth()};
	}

	private static Entity aimedEntity() {
		BlockHitResult hit = mc().result;
		return hit != null && hit.type == BlockHitResult.Type.ENTITY ? hit.entity : null;
	}

	@Override
	public int aimKind() {
		Entity e = aimedEntity();
		if (e instanceof PlayerEntity) {
			return 1;
		}
		if (e instanceof Monster) {
			return 2;
		}
		return e instanceof LivingEntity ? 3 : 0;
	}

	@Override
	public boolean hudHidden() {
		return mc().options.hudHidden || mc().options.debugEnabled;
	}

	@Override
	public boolean localMoving() {
		ClientPlayerEntity p = mc().player;
		return p != null && (p.input.movementForward != 0 || p.input.movementSideways != 0 || p.input.jumping || p.input.sneaking);
	}

	@Override
	public UUID worldPlayerId() {
		return mc().player == null ? null : mc().player.getUuid();
	}

	@Override
	public List<Object[]> otherPlayers() {
		List<Object[]> out = new ArrayList<Object[]>();
		MinecraftClient mc = mc();
		if (mc.world == null) {
			return out;
		}
		UUID self = worldPlayerId();
		for (PlayerEntity p : mc.world.playerEntities) {
			if (!p.getUuid().equals(self)) {
				out.add(new Object[] {p.getUuid(), p.getGameProfile().getName(), p.x, p.y, p.z});
			}
		}
		return out;
	}

	@Override
	public int[] light() {
		MinecraftClient mc = mc();
		if (mc.world == null || mc.player == null) {
			return null;
		}
		BlockPos pos = mc.player.getBlockPos();
		return new int[] {mc.world.getLightAtPos(LightType.BLOCK, pos), mc.world.getLightAtPos(LightType.SKY, pos)};
	}

	@Override
	public java.util.List<Object[]> arrows() {
		ClientPlayerEntity p = mc().player;
		if (p == null) {
			return java.util.Collections.emptyList();
		}
		java.util.Map<String, Object[]> kinds = new java.util.LinkedHashMap<String, Object[]>();
		for (ItemStack stack : mainSlots(p)) {
			if (!present(stack)) {
				continue;
			}
			Identifier id = (Identifier) net.minecraft.item.Item.REGISTRY.getIdentifier(stack.getItem());
			if (id == null || !id.getPath().endsWith("arrow")) {
				continue;
			}
			// The name tells tipped kinds apart ("Arrow of Poison").
			String kind = stack.getCustomName();
			Object[] row = kinds.get(kind);
			if (row == null) {
				kinds.put(kind, new Object[] {stack, count(stack)});
			} else {
				row[1] = (Integer) row[1] + count(stack);
			}
		}
		return new java.util.ArrayList<Object[]>(kinds.values());
	}

	@Override
	public int countItems(String idPart) {
		ClientPlayerEntity p = mc().player;
		if (p == null) {
			return -1;
		}
		int n = 0;
		for (ItemStack stack : mainSlots(p)) {
			if (present(stack)) {
				Identifier id = (Identifier) net.minecraft.item.Item.REGISTRY.getIdentifier(stack.getItem());
				if (id != null && id.getPath().contains(idPart)) {
					n += count(stack);
				}
			}
		}
		return n;
	}

	@Override
	public float[] food() {
		ClientPlayerEntity p = mc().player;
		return p == null ? null : new float[] {p.getHungerManager().getFoodLevel(), p.getHungerManager().getSaturationLevel()};
	}

	@Override
	public List<Object[]> durability() {
		ClientPlayerEntity p = mc().player;
		if (p == null) {
			return Collections.emptyList();
		}
		List<Object[]> rows = new ArrayList<Object[]>();
		String[] slots = {"feet", "legs", "chest", "head"};
		List<ItemStack> armor = armorSlots(p);
		for (int i = 0; i < armor.size(); i++) {
			addDurability(rows, armor.get(i), slots[Math.min(i, slots.length - 1)]);
		}
		addDurability(rows, p.inventory.getMainHandStack(), "mainhand");
		return rows;
	}

	private static void addDurability(List<Object[]> rows, ItemStack stack, String slot) {
		if (present(stack) && stack.isDamageable()) {
			rows.add(new Object[] {stack.getCustomName(), stack.getMaxDamage() - stack.getDamage(), stack.getMaxDamage(), slot});
		}
	}

	@Override
	public boolean dead() {
		ClientPlayerEntity p = mc().player;
		return p != null && p.getHealth() <= 0;
	}

	@Override
	public String resourcePack() {
		List<ResourcePackLoader.Entry> selected = mc().getResourcePackLoader().getSelectedResourcePacks();
		return selected.isEmpty() ? null : selected.get(selected.size() - 1).getName().replaceAll("\\.zip$", "");
	}

	@Override
	public File resourcePackDir() {
		return mc().getResourcePackLoader().getResourcePackDir();
	}

	@Override
	public void enableResourcePack(String fileName) {
		MinecraftClient mc = mc();
		ResourcePackLoader loader = mc.getResourcePackLoader();
		loader.initResourcePacks();
		for (ResourcePackLoader.Entry entry : loader.getAvailableResourcePacks()) {
			if (!entry.getName().equals(fileName)) {
				continue;
			}
			List<ResourcePackLoader.Entry> selected = new ArrayList<ResourcePackLoader.Entry>(loader.getSelectedResourcePacks());
			selected.remove(entry);
			selected.add(entry);
			loader.setSelectedResourcePacks(selected);
			// Same order as the loader's list: bottom to top.
			mc.options.resourcePacks.clear();
			for (ResourcePackLoader.Entry e : selected) {
				mc.options.resourcePacks.add(e.getName());
			}
			mc.options.save();
			mc.reloadResources();
			return;
		}
		ArcticLegacy.LOG.warn("pack {} isn't available after the download", fileName);
	}

	@Override
	public List<com.arcticlauncher.client.packs.PackInfo> resourcePacks() {
		ResourcePackLoader loader = mc().getResourcePackLoader();
		loader.initResourcePacks();
		// The loader's selected list runs bottom to top.
		List<ResourcePackLoader.Entry> selected = new ArrayList<ResourcePackLoader.Entry>(loader.getSelectedResourcePacks());
		List<com.arcticlauncher.client.packs.PackInfo> out = new ArrayList<com.arcticlauncher.client.packs.PackInfo>();
		for (int i = selected.size() - 1; i >= 0; i--) {
			out.add(info(selected.get(i), true));
		}
		List<com.arcticlauncher.client.packs.PackInfo> off = new ArrayList<com.arcticlauncher.client.packs.PackInfo>();
		for (ResourcePackLoader.Entry e : loader.getAvailableResourcePacks()) {
			if (!selected.contains(e)) {
				off.add(info(e, false));
			}
		}
		off.sort((a, b) -> a.title.compareToIgnoreCase(b.title));
		out.addAll(off);
		return out;
	}

	private com.arcticlauncher.client.packs.PackInfo info(ResourcePackLoader.Entry e, boolean on) {
		String name = e.getName();
		String title = name.toLowerCase(java.util.Locale.ROOT).endsWith(".zip") ? name.substring(0, name.length() - 4) : name;
		return new com.arcticlauncher.client.packs.PackInfo(name, title, e.getDescription(), on, false, new java.io.File(resourcePackDir(), name));
	}

	@Override
	public void setResourcePacks(List<String> enabledTopFirst) {
		MinecraftClient mc = mc();
		ResourcePackLoader loader = mc.getResourcePackLoader();
		loader.initResourcePacks();
		List<ResourcePackLoader.Entry> bottomFirst = new ArrayList<ResourcePackLoader.Entry>();
		for (int i = enabledTopFirst.size() - 1; i >= 0; i--) {
			for (ResourcePackLoader.Entry e : loader.getAvailableResourcePacks()) {
				if (e.getName().equals(enabledTopFirst.get(i))) {
					bottomFirst.add(e);
				}
			}
		}
		loader.setSelectedResourcePacks(bottomFirst);
		mc.options.resourcePacks.clear();
		for (ResourcePackLoader.Entry e : bottomFirst) {
			mc.options.resourcePacks.add(e.getName());
		}
		mc.options.save();
		mc.reloadResources();
	}

	@Override
	public String worldKey() {
		MinecraftClient mc = mc();
		if (mc.world == null) {
			return null;
		}
		if (mc.getServer() != null) {
			return "sp:" + mc.getServer().getLevelName();
		}
		ServerInfo server = mc.getCurrentServerEntry();
		return server == null ? "unknown" : server.address.toLowerCase(java.util.Locale.ROOT);
	}

	@Override
	public String dimension() {
		ClientPlayerEntity p = mc().player;
		if (p == null) {
			return "overworld";
		}
		switch (p.dimension) {
			case -1:
				return "the_nether";
			case 1:
				return "the_end";
			default:
				return "overworld";
		}
	}

	@Override
	public int playerCount() {
		return mc().getNetworkHandler() == null ? -1 : mc().getNetworkHandler().getPlayerList().size();
	}

	@Override
	public int entityCount() {
		return mc().world == null ? -1 : mc().world.loadedEntities.size();
	}

	@Override
	public Object connectionKey() {
		return mc().getNetworkHandler();
	}

	@Override
	public void sendChat(String text) {
		if (mc().player != null && text != null && !text.isEmpty()) {
			mc().player.sendChatMessage(text);
		}
	}

	@Override
	public void mentionSound() {
		//#if MC >= 1.9
		mc().getSoundManager().play(PositionedSoundInstance.method_12521(net.minecraft.sound.Sounds.ENTITY_EXPERIENCE_ORB_PICKUP, MENTION_PITCH));
		//#else
		mc().getSoundManager().play(PositionedSoundInstance.master(new Identifier("random.orb"), MENTION_PITCH));
		//#endif
	}

	// ---- Menus ------------------------------------------------------------------------

	@Override
	public void proxyChanged(com.arcticlauncher.client.config.ProxyConfig proxy) {
		com.arcticlauncher.client.net.ProxyRoutes.set(proxy);
	}

	@Override
	public void runOnGameThread(Runnable r) {
		LegacyHooks.onNextFrame(r);
	}

	@Override
	public boolean controlDown() {
		return Screen.hasControlDown();
	}

	@Override
	public void setClipboard(String text) {
		Screen.setClipboard(text);
	}

	@Override
	public String clipboard() {
		String clip = Screen.getClipboard();
		return clip == null ? "" : clip;
	}

	@Override
	public void openPage(Page page) {
		// One Arctic page replaces another, so Escape goes back to what was
		// under the menu instead of through every page visited.
		Screen current = mc().currentScreen;
		Screen under = current instanceof LegacyPageScreen ? ((LegacyPageScreen) current).parent() : current;
		mc().setScreen(new LegacyPageScreen(page, under));
	}

	@Override
	public void closePage() {
		if (mc().currentScreen instanceof LegacyPageScreen) {
			((LegacyPageScreen) mc().currentScreen).close();
		}
	}

	@Override
	public void action(MenuAction action) {
		MinecraftClient mc = mc();
		Screen parent = mc.currentScreen;
		switch (action) {
			case SINGLEPLAYER:
				mc.setScreen(new SelectWorldScreen(parent));
				break;
			case MULTIPLAYER:
			case REALMS:
				mc.setScreen(new MultiplayerScreen(parent));
				break;
			case OPTIONS:
			case ACCESSIBILITY:
				mc.setScreen(new SettingsScreen(parent, mc.options));
				break;
			case LANGUAGE:
				mc.setScreen(new LanguageOptionsScreen(parent, mc.options, mc.getLanguageManager()));
				break;
			default:
				mc.scheduleStop();
				break;
		}
	}

	@Override
	public double[] camera() {
		return LegacyHooks.camera();
	}

	// ---- Duels --------------------------------------------------------------------------

	private static final String DUEL_PREFIX = "Arctic Duel ";

	@Override
	public boolean canDuel() {
		return true;
	}

	@Override
	public boolean oldCommands() {
		return true;
	}

	/** A flat survival world with commands on (one at a time: earlier duel arenas are thrown away). */
	@Override
	public void createDuelWorld() {
		LegacyHooks.onNextFrame(() -> {
			MinecraftClient mc = mc();
			leave(mc);
			LegacyWorldTest.deleteWorlds(new File(mc.runDirectory, "saves"), DUEL_PREFIX);
			String name = DUEL_PREFIX + new java.text.SimpleDateFormat("MM-dd HH.mm").format(new java.util.Date());
			//#if MC >= 1.10
			net.minecraft.world.level.LevelInfo info = new net.minecraft.world.level.LevelInfo(System.currentTimeMillis(),
					net.minecraft.world.GameMode.SURVIVAL, false, false, net.minecraft.world.level.LevelGeneratorType.FLAT)
			//#else
			net.minecraft.world.level.LevelInfo info = new net.minecraft.world.level.LevelInfo(System.currentTimeMillis(),
					net.minecraft.world.level.LevelInfo.GameMode.SURVIVAL, false, false, net.minecraft.world.level.LevelGeneratorType.FLAT)
			//#endif
					.enableCommands();
			mc.startIntegratedServer(name, name, info);
		});
	}

	@Override
	public boolean inSingleplayerWorld() {
		MinecraftClient mc = mc();
		return mc.world != null && mc.player != null && mc.getServer() != null;
	}

	@Override
	public boolean openToLan() {
		net.minecraft.server.integrated.IntegratedServer server = mc().getServer();
		if (server == null) {
			return false;
		}
		if (server.isPublished()) {
			return true;
		}
		//#if MC >= 1.10
		return server.openToLAN(net.minecraft.world.GameMode.SURVIVAL, true) != null;
		//#else
		return server.getPort(net.minecraft.world.level.LevelInfo.GameMode.SURVIVAL, true) != null;
		//#endif
	}

	@Override
	public void leaveWorld() {
		MinecraftClient mc = mc();
		leave(mc);
		mc.setScreen(new TitleScreen());
	}

	/** How long to wait at most for a singleplayer server to stop after quitting it. */
	private static final long SERVER_STOP_WAIT_MS = 10_000;

	/**
	 * Quit the world. A singleplayer server stops by itself once you quit it;
	 * {@code connect(null)} asks it to stop too and waits for a task it hands
	 * the server, which never runs once the server is already shutting down. So
	 * wait for the server to finish stopping first (it then runs that task at once).
	 */
	public static void leave(MinecraftClient mc) {
		if (mc.world == null) {
			return;
		}
		net.minecraft.server.integrated.IntegratedServer server = mc.getServer();
		mc.world.disconnect();
		if (server != null) {
			long deadline = System.currentTimeMillis() + SERVER_STOP_WAIT_MS;
			while (!server.isStopped() && System.currentTimeMillis() < deadline) {
				try {
					Thread.sleep(10);
				} catch (InterruptedException e) {
					Thread.currentThread().interrupt();
					break;
				}
			}
		}
		mc.connect(null);
	}

	@Override
	public boolean connectTo(String address) {
		MinecraftClient mc = mc();
		LegacyHooks.onNextFrame(() -> {
			leave(mc);
			mc.setScreen(new ConnectScreen(new TitleScreen(), mc, new ServerInfo(address, address, false)));
		});
		return true;
	}

	// ---- Account and looks ---------------------------------------------------------------

	@Override
	public UUID playerId() {
		return mc().getSession().getProfile().getId();
	}

	@Override
	public String playerName() {
		return mc().getSession().getUsername();
	}

	/**
	 * Play as another account without restarting: on these versions the session is all there is per
	 * account (server joins read its token, the profile comes from it); the old account's profile
	 * properties (its skin) are dropped.
	 */
	@Override
	public String switchAccount(String name, UUID uuid, String accessToken, String xuid, boolean microsoft) {
		MinecraftClient mc = mc();
		if (mc.world != null) {
			return "leave the world first";
		}
		String undashed = uuid.toString().replace("-", "");
		((com.arcticlauncher.legacy.mixin.MinecraftClientAccountAccess) mc).arctic$setSession(
				new net.minecraft.client.util.Session(name, undashed, accessToken, microsoft ? "mojang" : "legacy"));
		mc.getSessionProperties().clear();
		ArcticLegacy.LOG.info("Now playing as {}", name);
		return null;
	}

	@Override
	public void joinServer(String serverId) throws Exception {
		mc().getSessionService().joinServer(mc().getSession().getProfile(), mc().getSession().getAccessToken(), serverId);
	}

	@Override
	public void registerTexture(String hash, byte[] png, boolean cape) {
		BufferedImage image = LegacyTextures.decode(png);
		if (image == null) {
			return;
		}
		int frames = cape ? Looks.capeFrames(image.getWidth(), image.getHeight()) : 1;
		mc().submit(() -> {
			if (frames < 2) {
				if (LegacyTextures.register(LegacyTextures.look(hash), image)) {
					ArcticClient.looks().textureReady(hash, 1);
				}
				return;
			}
			int h = image.getHeight() / frames;
			boolean ok = true;
			for (int f = 0; f < frames; f++) {
				ok &= LegacyTextures.register(LegacyTextures.look(hash + "/" + f), image.getSubimage(0, f * h, image.getWidth(), h));
			}
			if (ok) {
				ArcticClient.looks().textureReady(hash, frames);
			}
		});
	}

	@Override
	public void registerCosmetic(String id, com.arcticlauncher.client.looks.Geometry geometry, byte[] png, byte[] glowPng) {
		com.arcticlauncher.client.looks.Cosmetics.Item item = ArcticClient.looks().cosmetics().item(id);
		LegacyCosmetics.register(id, geometry, item == null ? null : item.idle, png, glowPng);
	}

	@Override
	public void registerMesh(String id, com.arcticlauncher.client.looks.MeshModel mesh) {
		LegacyCosmetics.register(id, mesh);
	}

	@Override
	public double fovDegrees() {
		return net.minecraft.client.MinecraftClient.getInstance().options.fov;
	}

	@Override
	public boolean smoothCamera() {
		return net.minecraft.client.MinecraftClient.getInstance().options.smoothCameraEnabled;
	}

	@Override
	public void smoothCamera(boolean on) {
		net.minecraft.client.MinecraftClient.getInstance().options.smoothCameraEnabled = on;
	}
}
