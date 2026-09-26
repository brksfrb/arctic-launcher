package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.GameKey;
import com.arcticlauncher.client.MenuAction;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.looks.Looks;
import com.arcticlauncher.client.ui.Page;
import com.mojang.blaze3d.platform.NativeImage;
import com.mojang.realmsclient.RealmsMainScreen;
import java.io.File;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.CameraType;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.User;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;
import net.minecraft.client.gui.screens.multiplayer.SafetyScreen;
import net.minecraft.client.gui.screens.options.AccessibilityOptionsScreen;
import net.minecraft.client.gui.screens.options.LanguageSelectScreen;
import net.minecraft.client.gui.screens.options.OptionsScreen;
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
		return mc().getFps();
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
		return new double[] {p.getX(), p.getY(), p.getZ(), p.getYRot(), p.getXRot()};
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
		String id = Compat.biomeId(mc.level, mc.player.blockPosition());
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

	/** The camera before Freelook switched to third person. */
	private CameraType before;

	@Override
	public void setThirdPerson(boolean on) {
		Options options = mc().options;
		if (on) {
			before = options.getCameraType();
			options.setCameraType(CameraType.THIRD_PERSON_BACK);
		} else if (before != null) {
			options.setCameraType(before);
			before = null;
		}
	}

	@Override
	public boolean hasFeatures() {
		return Compat.FEATURES;
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
	public Object[] target() {
		return GameInfo.target();
	}

	@Override
	public boolean hudHidden() {
		return Compat.hudHidden() || mc().getDebugOverlay().showDebugScreen();
	}

	@Override
	public void openPage(Page page) {
		Compat.setScreen(new PageScreen(page, Compat.screen()));
	}

	@Override
	public void closePage() {
		if (Compat.screen() instanceof PageScreen screen) {
			screen.onClose();
		}
	}

	@Override
	public void action(MenuAction action) {
		Minecraft mc = mc();
		Screen parent = Compat.screen();
		switch (action) {
			case SINGLEPLAYER -> Compat.setScreen(new SelectWorldScreen(parent));
			case MULTIPLAYER -> Compat.setScreen(mc.options.skipMultiplayerWarning
					? new JoinMultiplayerScreen(parent)
					: new SafetyScreen(parent));
			case REALMS -> Compat.setScreen(new RealmsMainScreen(parent));
			case OPTIONS -> Compat.setScreen(Compat.optionsScreen(parent));
			case LANGUAGE -> Compat.setScreen(new LanguageSelectScreen(parent, mc.options, mc.getLanguageManager()));
			case ACCESSIBILITY -> Compat.setScreen(new AccessibilityOptionsScreen(parent, mc.options));
			case QUIT -> mc.stop();
		}
	}

	@Override
	public UUID playerId() {
		return mc().getUser().getProfileId();
	}

	@Override
	public String playerName() {
		return mc().getUser().getName();
	}

	@Override
	public void registerTexture(String hash, byte[] png, boolean cape) {
		try {
			NativeImage image = NativeImage.read(png);
			int frames = cape ? Looks.capeFrames(image.getWidth(), image.getHeight()) : 1;
			if (frames < 2) {
				mc().execute(() -> {
					register(hash, image);
					ArcticClient.looks().textureReady(hash, 1);
				});
				return;
			}
			List<NativeImage> parts = splitFrames(image, frames);
			image.close();
			mc().execute(() -> {
				for (int i = 0; i < parts.size(); i++) {
					register(hash + "/" + i, parts.get(i));
				}
				ArcticClient.looks().textureReady(hash, parts.size());
			});
		} catch (Exception e) {
			ArcticMod.LOG.debug("texture {}: {}", hash, e.toString());
		}
	}

	private static void register(String key, NativeImage image) {
		mc().getTextureManager().register(GfxImpl.look(key), Compat.texture("Arctic " + key, image));
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
					part.setPixel(x, y, strip.getPixel(x, f * h + y));
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
	public List<Object[]> otherPlayers() {
		List<Object[]> out = new ArrayList<>();
		Minecraft mc = mc();
		if (mc.level == null) {
			return out;
		}
		UUID self = playerId();
		for (AbstractClientPlayer p : mc.level.players()) {
			if (!p.getUUID().equals(self)) {
				out.add(new Object[] {p.getUUID(), Compat.playerName(p)});
			}
		}
		return out;
	}
}
