package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.GameKey;
import com.arcticlauncher.client.MenuAction;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.looks.Looks;
import com.arcticlauncher.client.ui.Page;
import com.mojang.blaze3d.platform.NativeImage;
//#if MC >= 1.16
import com.mojang.realmsclient.RealmsMainScreen;
//#endif
import java.io.File;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.User;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;
import net.minecraft.client.gui.screens.multiplayer.SafetyScreen;
//#if MC >= 1.21
import net.minecraft.client.gui.screens.options.AccessibilityOptionsScreen;
import net.minecraft.client.gui.screens.options.LanguageSelectScreen;
import net.minecraft.client.gui.screens.options.OptionsScreen;
//#else
import net.minecraft.client.gui.screens.AccessibilityOptionsScreen;
import net.minecraft.client.gui.screens.LanguageSelectScreen;
import net.minecraft.client.gui.screens.OptionsScreen;
//#endif
import net.minecraft.client.gui.screens.worldselection.SelectWorldScreen;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.texture.DynamicTexture;

/** {@link Platform} for Minecraft 26.3. */
final class FabricPlatform implements Platform {
	private static Minecraft mc() {
		return Minecraft.getInstance();
	}

	@Override
	public String minecraftVersion() {
		return FabricLoader.getInstance()
				.getModContainer("minecraft")
				.map(c -> c.getMetadata().getVersion().getFriendlyString())
				.orElse("26.3");
	}

	@Override
	public File configDir() {
		return FabricLoader.getInstance().getConfigDir().toFile();
	}

	@Override
	public void log(boolean warning, String message) {
		if (warning) {
			ArcticMod.LOG.warn(message);
		} else {
			ArcticMod.LOG.info(message);
		}
	}

	@Override
	public int fps() {
		return Compat.fps();
	}

	@Override
	public int ping() {
		Minecraft mc = mc();
		ClientPacketListener connection = mc.getConnection();
		if (connection == null || mc.player == null || mc.hasSingleplayerServer()) {
			return -1;
		}
		PlayerInfo info = connection.getPlayerInfo(mc.player.getUUID());
		return info == null ? -1 : info.getLatency();
	}

	@Override
	public boolean inWorld() {
		return mc().level != null && mc().player != null;
	}

	@Override
	public double[] position() {
		LocalPlayer p = mc().player;
		if (p == null) {
			return null;
		}
		return new double[] {p.getX(), p.getY(), p.getZ(), Compat.yRot(p), Compat.xRot(p)};
	}

	@Override
	public boolean keyDown(GameKey key) {
		return mapping(mc().options, key).isDown();
	}

	private static KeyMapping mapping(Options o, GameKey key) {
		switch (key) {
			case FORWARD:
				return o.keyUp;
			case BACK:
				return o.keyDown;
			case LEFT:
				return o.keyLeft;
			case RIGHT:
				return o.keyRight;
			case JUMP:
				return o.keyJump;
			case SNEAK:
				return o.keyShift;
			case SPRINT:
				return o.keySprint;
			case ATTACK:
				return o.keyAttack;
			default:
				return o.keyUse;
		}
	}

	@Override
	public String biome() {
		Minecraft mc = mc();
		if (mc.level == null || mc.player == null) {
			return null;
		}
		String id = Compat.biomeId(mc.level, Compat.blockPos(mc.player));
		return id == null ? null : title(id);
	}

	/** "snowy_plains" → "Snowy Plains". */
	private static String title(String id) {
		StringBuilder out = new StringBuilder();
		for (String word : id.split("_")) {
			if (!word.isEmpty()) {
				out.append(out.length() == 0 ? "" : " ").append(Character.toUpperCase(word.charAt(0))).append(word.substring(1));
			}
		}
		return out.toString();
	}

	@Override
	public String server() {
		Minecraft mc = mc();
		if (mc.level == null) {
			return null;
		}
		if (mc.hasSingleplayerServer() || mc.getCurrentServer() == null) {
			return "Singleplayer";
		}
		return mc.getCurrentServer().ip;
	}

	@Override
	public long dayTime() {
		return mc().level == null ? -1 : Compat.dayTime(mc().level);
	}

	@Override
	public boolean isKeyDown(String key) {
		return Compat.keyDown(key);
	}

	@Override
	public String keyLabel(String key) {
		return Compat.keyLabel(key);
	}

	@Override
	public String keyName(int nativeKey) {
		return Compat.keyName(nativeKey);
	}

	/** The camera before Freelook switched to third person (-1: none). */
	private int before = -1;

	@Override
	public void setThirdPerson(boolean on) {
		if (on) {
			before = Compat.camera();
			Compat.setCamera(1);
		} else if (before >= 0) {
			Compat.setCamera(before);
			before = -1;
		}
	}

	@Override
	public boolean hasFeatures() {
		return Compat.FEATURES;
	}

	@Override
	public boolean smoothFont() {
		//#if MC >= 26.1
		return ArcticPacks.smoothFontOn();
		//#else
		return false;
		//#endif
	}

	@Override
	public void setSmoothFont(boolean on) {
		//#if MC >= 26.1
		ArcticPacks.setSmoothFont(on);
		//#endif
	}

	@Override
	public boolean physicalKeyDown(GameKey key) {
		return Compat.keyDown(mapping(mc().options, key).saveString());
	}

	@Override
	public void setKeyDown(GameKey key, boolean down) {
		mapping(mc().options, key).setDown(down);
	}

	@Override
	public int hurtTime() {
		return GameInfo.hurtTime();
	}

	@Override
	public List<Object[]> armor() {
		return GameInfo.armor();
	}

	@Override
	public List<Object[]> effects() {
		return GameInfo.effects();
	}

	@Override
	public Object[] heldItem() {
		return GameInfo.heldItem();
	}

	@Override
	public void registerCosmetic(String id, com.arcticlauncher.client.looks.Geometry geometry, byte[] png) {
		//#if MC >= 26.1
		NativeImage image;
		try {
			image = NativeImage.read(new java.io.ByteArrayInputStream(png));
		} catch (Exception e) {
			ArcticMod.LOG.debug("cosmetic {}: {}", id, e.toString());
			return;
		}
		com.arcticlauncher.client.looks.Cosmetics.Item item = ArcticClient.looks().cosmetics().item(id);
		mc().execute(() -> {
			try {
				net.minecraft.resources.Identifier texture = GfxImpl.look("cosmetic/" + id);
				mc().getTextureManager().register(texture, Compat.texture("Arctic cosmetic " + id, image));
				com.arcticlauncher.mod.cosmetic.CosmeticModels.bake(id, geometry, item == null ? null : item.idle, texture);
				ArcticClient.looks().cosmetics().ready(id);
			} catch (Exception e) {
				ArcticMod.LOG.warn("cosmetic {}: {}", id, e.toString());
			}
		});
		//#endif
	}

	@Override
	public void runOnGameThread(Runnable r) {
		mc().execute(r);
	}

	@Override
	public String switchAccount(String name, UUID uuid, String accessToken, String xuid, boolean microsoft) {
		return AccountSwap.swap(name, uuid, accessToken, xuid, microsoft);
	}

	@Override
	public boolean localMoving() {
		if (mc().player == null) {
			return false;
		}
		//#if MC >= 1.21.2
		net.minecraft.world.entity.player.Input keys = mc().player.input.keyPresses;
		return keys.forward() || keys.backward() || keys.left() || keys.right() || keys.jump() || keys.shift();
		//#else
		// Before 1.21.2 input was a plain Input with boolean fields, not a ClientInput/record.
		net.minecraft.client.player.Input keys = mc().player.input;
		return keys.up || keys.down || keys.left || keys.right || keys.jumping || keys.shiftKeyDown;
		//#endif
	}

	@Override
	public UUID worldPlayerId() {
		return mc().player == null ? null : mc().player.getUUID();
	}

	@Override
	public boolean controlDown() {
		//#if MC >= 1.21.9
		return mc().hasControlDown();
		//#else
		return net.minecraft.client.gui.screens.Screen.hasControlDown();
		//#endif
	}

	@Override
	public void setClipboard(String text) {
		mc().keyboardHandler.setClipboard(text);
	}

	@Override
	public String clipboard() {
		String clip = mc().keyboardHandler.getClipboard();
		return clip == null ? "" : clip;
	}

	@Override
	public void textInput(Object owner, boolean on) {
		//#if MC >= 26.3
		if (on) {
			mc().textInputManager().startTextInput(owner);
		} else {
			mc().textInputManager().stopTextInput(owner);
		}
		//#elif MC >= 26.1
		if (on) {
			mc().textInputManager().startTextInput();
		} else {
			mc().textInputManager().stopTextInput();
		}
		//#endif
	}

	@Override
	public void proxyChanged(com.arcticlauncher.client.config.ProxyConfig proxy) {
		com.arcticlauncher.client.net.ProxyRoutes.set(proxy);
	}

	@Override
	public Object sampleItem(String id) {
		return GameInfo.sampleItem(id);
	}

	@Override
	public Object effectSprite(String id) {
		return GameInfo.effectSprite(id);
	}

	@Override
	public Object[] target() {
		return GameInfo.target();
	}

	@Override
	public boolean hudHidden() {
		return Compat.hudHidden() || Compat.debugScreenShowing();
	}

	@Override
	public void openPage(Page page) {
		// One Arctic page replaces another, so Escape goes back to what was
		// under the menu instead of through every page visited.
		Screen now = Compat.screen();
		Screen under = now instanceof PageScreen ? ((PageScreen) now).parent() : now;
		Compat.setScreen(new PageScreen(page, under));
	}

	@Override
	public void closePage() {
		if (Compat.screen() instanceof PageScreen) {
			((PageScreen) Compat.screen()).onClose();
		}
	}

	@Override
	public void action(MenuAction action) {
		Minecraft mc = mc();
		Screen parent = Compat.screen();
		switch (action) {
			case SINGLEPLAYER:
				Compat.setScreen(new SelectWorldScreen(parent));
				break;
			case MULTIPLAYER:
				Compat.setScreen(mc.options.skipMultiplayerWarning ? new JoinMultiplayerScreen(parent) : new SafetyScreen(parent));
				break;
			case REALMS:
				//#if MC >= 1.16
				Compat.setScreen(new RealmsMainScreen(parent));
				//#else
				new net.minecraft.realms.RealmsBridge().switchToRealms(parent);
				//#endif
				break;
			case OPTIONS:
				Compat.setScreen(Compat.optionsScreen(parent));
				break;
			case LANGUAGE:
				Compat.setScreen(new LanguageSelectScreen(parent, mc.options, mc.getLanguageManager()));
				break;
			case ACCESSIBILITY:
				Compat.setScreen(new AccessibilityOptionsScreen(parent, mc.options));
				break;
			case QUIT:
				mc.stop();
				break;
			default:
				break;
		}
	}

	@Override
	public UUID playerId() {
		//#if MC >= 1.19
		return mc().getUser().getProfileId();
		//#else
		return mc().getUser().getGameProfile().getId();
		//#endif
	}

	@Override
	public String playerName() {
		return mc().getUser().getName();
	}

	@Override
	public void registerTexture(String hash, byte[] png, boolean cape) {
		// Looks already checked the hash and the PNG's size; the decode itself
		// can still fail on damaged data, which just means no texture.
		NativeImage image;
		try {
			image = NativeImage.read(new java.io.ByteArrayInputStream(png));
		} catch (Exception e) {
			ArcticMod.LOG.debug("texture {}: {}", hash, e.toString());
			return;
		}
		int frames = cape ? Looks.capeFrames(image.getWidth(), image.getHeight()) : 1;
		if (frames < 2) {
			mc().execute(() -> {
				if (register(hash, image)) {
					ArcticClient.looks().textureReady(hash, 1);
				}
			});
			return;
		}
		List<NativeImage> parts = splitFrames(image, frames);
		image.close();
		mc().execute(() -> {
			boolean ok = true;
			for (int i = 0; i < parts.size(); i++) {
				ok &= register(hash + "/" + i, parts.get(i));
			}
			if (ok) {
				ArcticClient.looks().textureReady(hash, parts.size());
			}
		});
	}

	/** Upload on the render thread; a failure is logged, never thrown into the game loop. */
	private static boolean register(String key, NativeImage image) {
		try {
			mc().getTextureManager().register(GfxImpl.look(key), Compat.texture("Arctic " + key, image));
			return true;
		} catch (Exception e) {
			image.close();
			ArcticMod.LOG.warn("texture {}: {}", key, e.toString());
			return false;
		}
	}

	/** An animated cape's stacked 2:1 frames as separate images. */
	private static List<NativeImage> splitFrames(NativeImage strip, int frames) {
		int w = strip.getWidth();
		int h = strip.getHeight() / frames;
		List<NativeImage> parts = new ArrayList<>();
		for (int f = 0; f < frames; f++) {
			NativeImage part = new NativeImage(w, h, true);
			for (int y = 0; y < h; y++) {
				for (int x = 0; x < w; x++) {
					//#if MC >= 1.21.2
					part.setPixel(x, y, strip.getPixel(x, f * h + y));
					//#else
					// Before 1.21.2 NativeImage's pixel accessors were named RGBA.
					part.setPixelRGBA(x, y, strip.getPixelRGBA(x, f * h + y));
					//#endif
				}
			}
			parts.add(part);
		}
		return parts;
	}

	@Override
	public void joinServer(String serverId) throws Exception {
		Compat.joinServer(serverId);
	}

	@Override
	public int aimKind() {
		Minecraft mc = mc();
		if (!(mc.hitResult instanceof net.minecraft.world.phys.EntityHitResult)) {
			return 0;
		}
		net.minecraft.world.phys.EntityHitResult hit = (net.minecraft.world.phys.EntityHitResult) mc.hitResult;
		net.minecraft.world.entity.Entity e = hit.getEntity();
		if (e instanceof net.minecraft.world.entity.player.Player) {
			return 1;
		}
		if (e instanceof net.minecraft.world.entity.monster.Enemy) {
			return 2;
		}
		return e instanceof net.minecraft.world.entity.LivingEntity ? 3 : 0;
	}

	/** Leaves the current world with no confirmation dialog, whatever this version calls that. */
	@Override
	public double[] camera() {
		Minecraft mc = mc();
		if (mc.level == null) {
			return null;
		}
		//#if MC >= 26.2
		net.minecraft.client.Camera cam = mc.gameRenderer.mainCamera();
		//#else
		net.minecraft.client.Camera cam = mc.gameRenderer.getMainCamera();
		//#endif
		//#if MC >= 1.21.11
		net.minecraft.world.phys.Vec3 at = cam.position();
		double[] out = {at.x, at.y, at.z, cam.yRot(), cam.xRot(), 0};
		//#else
		net.minecraft.world.phys.Vec3 at = cam.getPosition();
		double[] out = {at.x, at.y, at.z, cam.getYRot(), cam.getXRot(), 0};
		//#endif
		//#if MC >= 26.1
		out[5] = cam.getFov();
		//#else
		out[5] = Compat.worldFov;
		//#endif
		return out;
	}

	@Override
	public void leaveWorld() {
		Minecraft mc = mc();
		leaveWorld(mc);
		Compat.setScreen(new net.minecraft.client.gui.screens.TitleScreen());
	}

	private static void leaveWorld(Minecraft mc) {
		if (mc.level == null) {
			return;
		}
		Compat.quitLevel();
		//#if MC >= 1.21.9
		mc.disconnectFromWorld(com.arcticlauncher.mod.Compat.empty());
		//#elif MC >= 1.21.6
		mc.disconnectWithSavingScreen();
		//#elif MC >= 1.20.2
		mc.disconnect();
		//#else
		mc.clearLevel();
		//#endif
	}

	@Override
	public boolean connectTo(String address) {
		Minecraft mc = mc();
		mc.execute(() -> {
			leaveWorld(mc);
			//#if MC >= 1.20.2
			net.minecraft.client.multiplayer.ServerData data = new net.minecraft.client.multiplayer.ServerData(
					address, address, net.minecraft.client.multiplayer.ServerData.Type.OTHER);
			//#else
			// Before 1.20.2 ServerData's third argument was a plain "is this a LAN entry" boolean.
			net.minecraft.client.multiplayer.ServerData data =
					new net.minecraft.client.multiplayer.ServerData(address, address, false);
			//#endif
			//#if MC < 1.17
			mc.setScreen(new net.minecraft.client.gui.screens.ConnectScreen(new net.minecraft.client.gui.screens.TitleScreen(), mc, data));
			//#elif MC < 1.20
			net.minecraft.client.gui.screens.ConnectScreen.startConnecting(new net.minecraft.client.gui.screens.TitleScreen(), mc,
					net.minecraft.client.multiplayer.resolver.ServerAddress.parseString(address), data);
			//#elif MC >= 1.20.6
			net.minecraft.client.gui.screens.ConnectScreen.startConnecting(
					new net.minecraft.client.gui.screens.TitleScreen(), mc,
					net.minecraft.client.multiplayer.resolver.ServerAddress.parseString(address), data, false, null);
			//#else
			// Before 1.20.6 startConnecting had no trailing TransferState parameter.
			net.minecraft.client.gui.screens.ConnectScreen.startConnecting(
					new net.minecraft.client.gui.screens.TitleScreen(), mc,
					net.minecraft.client.multiplayer.resolver.ServerAddress.parseString(address), data, false);
			//#endif
		});
		return true;
	}

	@Override
	public boolean canSimpleVoiceChat() {
		//#if MC >= 1.20.5
		return com.arcticlauncher.mod.svc.SvcPayload.ENABLED;
		//#else
		// Before 1.20.5 custom payloads weren't CustomPacketPayload-based (plain
		// FriendlyByteBuf + channel ResourceLocation instead), no hook for this.
		return false;
		//#endif
	}

	@Override
	public void requestSimpleVoiceChat() {
		//#if MC >= 1.20.5
		Minecraft mc = mc();
		mc.execute(() -> {
			ClientPacketListener connection = mc.getConnection();
			if (connection != null) {
				for (com.arcticlauncher.mod.svc.SvcPayload p : com.arcticlauncher.mod.svc.SvcPayload.request()) {
					connection.send(new net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket(p));
				}
			}
		});
		//#endif
	}

	@Override
	public java.util.List<Object[]> durability() {
		return GameInfo.durability();
	}

	@Override
	public void mentionSound() {
		mc().getSoundManager().play(net.minecraft.client.resources.sounds.SimpleSoundInstance.forUI(
				net.minecraft.sounds.SoundEvents.EXPERIENCE_ORB_PICKUP, PING_PITCH, PING_VOLUME));
	}

	private static final float PING_PITCH = 1.6f;
	private static final float PING_VOLUME = 0.5f;

	@Override
	public boolean minimapWorks() {
		return scoreboardTweaks();
	}

	@Override
	public String minimap() {
		return MinimapTexture.refresh() ? "dyn:minimap" : null;
	}

	@Override
	public java.io.File resourcePackDir() {
		return new java.io.File(mc().gameDirectory, "resourcepacks");
	}

	@Override
	public void enableResourcePack(String fileName) {
		WorldStats.enablePack(fileName);
	}

	@Override
	public java.util.List<com.arcticlauncher.client.packs.PackInfo> resourcePacks() {
		return WorldStats.packs(resourcePackDir());
	}

	@Override
	public void setResourcePacks(java.util.List<String> enabledTopFirst) {
		WorldStats.setPacks(enabledTopFirst);
	}

	@Override
	public boolean canDuel() {
		return scoreboardTweaks();
	}

	@Override
	public void createDuelWorld() {
		//#if MC >= 1.19.4
		Minecraft mc = mc();
		mc.execute(() -> {
			leaveWorld(mc);
			// One duel world at a time: earlier ones are thrown away (they're only arenas).
			WorldStats.deleteWorlds(new java.io.File(mc.gameDirectory, "saves"), "Arctic Duel ");
			String name = "Arctic Duel " + new java.text.SimpleDateFormat("MM-dd HH.mm").format(new java.util.Date());
			//#if MC >= 26.1
			net.minecraft.world.level.LevelSettings settings = new net.minecraft.world.level.LevelSettings(name,
					net.minecraft.world.level.GameType.SURVIVAL,
					new net.minecraft.world.level.LevelSettings.DifficultySettings(net.minecraft.world.Difficulty.EASY, false, false),
					true, net.minecraft.world.level.WorldDataConfiguration.DEFAULT);
			//#elif MC >= 1.21.11
			// GameRules moved to a gamerules subpackage in 1.21.11.
			net.minecraft.world.level.LevelSettings settings = new net.minecraft.world.level.LevelSettings(name,
					net.minecraft.world.level.GameType.SURVIVAL, false, net.minecraft.world.Difficulty.EASY, true,
					new net.minecraft.world.level.gamerules.GameRules(net.minecraft.world.flag.FeatureFlags.DEFAULT_FLAGS),
					net.minecraft.world.level.WorldDataConfiguration.DEFAULT);
			//#elif MC >= 1.21.3
			// Before 26.1 LevelSettings took hardcore/difficulty/GameRules directly, with
			// no DifficultySettings record; since 1.21.3 GameRules also needs a FeatureFlagSet.
			net.minecraft.world.level.LevelSettings settings = new net.minecraft.world.level.LevelSettings(name,
					net.minecraft.world.level.GameType.SURVIVAL, false, net.minecraft.world.Difficulty.EASY, true,
					new net.minecraft.world.level.GameRules(net.minecraft.world.flag.FeatureFlags.DEFAULT_FLAGS),
					net.minecraft.world.level.WorldDataConfiguration.DEFAULT);
			//#else
			// Before 1.21.3 GameRules had a plain no-argument constructor.
			net.minecraft.world.level.LevelSettings settings = new net.minecraft.world.level.LevelSettings(name,
					net.minecraft.world.level.GameType.SURVIVAL, false, net.minecraft.world.Difficulty.EASY, true,
					new net.minecraft.world.level.GameRules(), net.minecraft.world.level.WorldDataConfiguration.DEFAULT);
			//#endif
			//#if MC >= 1.21.3
			mc.createWorldOpenFlows().createFreshLevel(name, settings, net.minecraft.world.level.levelgen.WorldOptions.defaultWithRandomSeed(),
					registries -> registries.lookupOrThrow(net.minecraft.core.registries.Registries.WORLD_PRESET)
							.getOrThrow(net.minecraft.world.level.levelgen.presets.WorldPresets.FLAT).value().createWorldDimensions(),
					new net.minecraft.client.gui.screens.TitleScreen());
			//#elif MC >= 1.20.4
			// Before 1.21.3 the dimensions function took RegistryAccess (registryOrThrow),
			// not HolderLookup.Provider (lookupOrThrow); createFreshLevel already took a Screen.
			mc.createWorldOpenFlows().createFreshLevel(name, settings, net.minecraft.world.level.levelgen.WorldOptions.defaultWithRandomSeed(),
					registries -> registries.registryOrThrow(net.minecraft.core.registries.Registries.WORLD_PRESET)
							.getHolderOrThrow(net.minecraft.world.level.levelgen.presets.WorldPresets.FLAT).value().createWorldDimensions(),
					new net.minecraft.client.gui.screens.TitleScreen());
			//#else
			// 1.20.1's createFreshLevel had no trailing Screen parameter yet.
			mc.createWorldOpenFlows().createFreshLevel(name, settings, net.minecraft.world.level.levelgen.WorldOptions.defaultWithRandomSeed(),
					registries -> registries.registryOrThrow(net.minecraft.core.registries.Registries.WORLD_PRESET)
							.getHolderOrThrow(net.minecraft.world.level.levelgen.presets.WorldPresets.FLAT).value().createWorldDimensions());
			//#endif
		});
		//#endif
	}

	@Override
	public boolean inSingleplayerWorld() {
		Minecraft mc = mc();
		return mc.level != null && mc.player != null && mc.getSingleplayerServer() != null;
	}

	@Override
	public boolean openToLan() {
		net.minecraft.client.server.IntegratedServer server = mc().getSingleplayerServer();
		if (server == null) {
			return false;
		}
		int port = net.minecraft.util.HttpUtil.getAvailablePort();
		//#if MC >= 26.3
		return server.isPublished() || server.publishServer(net.minecraft.server.MinecraftServer.MultiplayerScope.LAN, true, port);
		//#elif MC >= 26.2
		return server.isPublished() || server.publishServer(net.minecraft.server.MinecraftServer.MultiplayerScope.LAN, port);
		//#else
		return server.isPublished() || server.publishServer(net.minecraft.world.level.GameType.SURVIVAL, true, port);
		//#endif
	}

	@Override
	public boolean hitColorWorks() {
		return scoreboardTweaks();
	}

	@Override
	public void setHitColor(int rgb) {
		//#if MC >= 1.20
		Minecraft mc = mc();
		mc.execute(() -> {
			net.minecraft.client.renderer.texture.DynamicTexture texture =
					((com.arcticlauncher.mod.mixin.OverlayTextureAccess) mc.gameRenderer.overlayTexture()).arctic$texture();
			com.mojang.blaze3d.platform.NativeImage pixels = texture.getPixels();
			if (pixels == null) {
				return;
			}
			int color = HIT_ALPHA | (rgb == 0 ? HIT_RED : rgb & 0xFFFFFF);
			// The top half of the 16x16 overlay is the hurt tint.
			for (int y = 0; y < 8; y++) {
				for (int x = 0; x < 16; x++) {
					//#if MC >= 1.21.2
					pixels.setPixel(x, y, color);
					//#else
					pixels.setPixelRGBA(x, y, color);
					//#endif
				}
			}
			texture.upload();
		});
		//#endif
	}

	/** The game's hurt tint: red at 70% (0xB2). */
	private static final int HIT_ALPHA = 0xB2000000;
	private static final int HIT_RED = 0xFF0000;

	@Override
	public String worldKey() {
		return WorldStats.worldKey();
	}

	@Override
	public String dimension() {
		return WorldStats.dimension();
	}

	@Override
	public boolean outlineTweaks() {
		//#if MC >= 26.2
		return true;
		//#else
		return false;
		//#endif
	}

	@Override
	public boolean scoreboardTweaks() {
		// The "extras" mixins (scoreboard, hit color, minimap, duels) are 1.20+ for now.
		//#if MC >= 1.20
		return true;
		//#else
		return false;
		//#endif
	}

	@Override
	public int toastsBottom() {
		//#if MC < 1.19
		// No slot tracking before 1.19.
		return 0;
		//#else
		//#if MC >= 26.2
		java.util.BitSet slots = ((com.arcticlauncher.mod.mixin.ToastManagerAccess) mc().gui.toastManager()).arctic$occupiedSlots();
		//#elif MC >= 1.21.3
		java.util.BitSet slots = ((com.arcticlauncher.mod.mixin.ToastManagerAccess) mc().getToastManager()).arctic$occupiedSlots();
		//#else
		// Before 1.21.3 the toast queue was ToastComponent, reached through getToasts().
		java.util.BitSet slots = ((com.arcticlauncher.mod.mixin.ToastManagerAccess) mc().getToasts()).arctic$occupiedSlots();
		//#endif
		return slots.length() * net.minecraft.client.gui.components.toasts.Toast.SLOT_HEIGHT;
		//#endif
	}

	@Override
	public int[] light() {
		return WorldStats.light();
	}

	@Override
	public int countItems(String idPart) {
		return WorldStats.countItems(idPart);
	}

	@Override
	public float[] food() {
		return WorldStats.food();
	}

	@Override
	public String resourcePack() {
		return WorldStats.resourcePack();
	}

	@Override
	public boolean dead() {
		return WorldStats.dead();
	}

	@Override
	public void sendChat(String text) {
		WorldStats.sendChat(text);
	}

	@Override
	public int playerCount() {
		ClientPacketListener connection = mc().getConnection();
		//#if MC >= 1.19.3
		return connection == null ? -1 : connection.getListedOnlinePlayers().size();
		//#else
		return connection == null ? -1 : connection.getOnlinePlayers().size();
		//#endif
	}

	@Override
	public Object connectionKey() {
		return mc().getConnection();
	}

	@Override
	public List<Object[]> otherPlayers() {
		List<Object[]> out = new ArrayList<>();
		Minecraft mc = mc();
		if (mc.level == null) {
			return out;
		}
		UUID self = playerId();
		for (AbstractClientPlayer p : mc.level.players()) {
			if (!p.getUUID().equals(self)) {
				out.add(new Object[] {p.getUUID(), Compat.playerName(p), p.getX(), p.getY(), p.getZ()});
			}
		}
		return out;
	}
}
