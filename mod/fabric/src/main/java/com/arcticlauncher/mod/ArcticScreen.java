package com.arcticlauncher.mod;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.UUID;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;

/**
 * The in-game Arctic menu: pick a cape, hide looks, or hide one player's
 * look. Skins are chosen in the launcher.
 */
public final class ArcticScreen extends Screen {
	private static final int MAX_COLUMNS = 6;
	private static final int TILE_W = 64;
	private static final int TILE_H = 88;
	private static final int GAP = 8;
	private static final int TOP = 50;
	private static final int ICE = 0xFF7DD3FC;
	private static final int MUTED = 0xFFA9BCD0;
	private static final int MAX_NEARBY = 4;

	private final Screen parent;
	private int seen = -1;

	public ArcticScreen(Screen parent) {
		super(Component.literal("Arctic"));
		this.parent = parent;
	}

	/** Button that opens the menu, for the title and pause screens. */
	public static Button openButton(Screen from) {
		return Button.builder(Component.literal("Arctic"), b -> Minecraft.getInstance().gui.setScreen(new ArcticScreen(from)))
				.bounds(4, 4, 64, 20)
				.build();
	}

	@Override
	protected void init() {
		if (Cosmetics.presets().isEmpty() && !Cosmetics.busy()) {
			Cosmetics.open();
		}
		rebuild();
	}

	/** (preset id, label, texture hash); the first is "no cape". */
	private List<String[]> tiles() {
		List<String[]> tiles = new ArrayList<>();
		tiles.add(new String[] {null, "No cape", null});
		for (Cosmetics.Preset p : Cosmetics.presets()) {
			tiles.add(new String[] {p.id(), p.name(), p.texture()});
		}
		return tiles;
	}

	private int columns(int count) {
		int fit = Math.max(1, (width - 20 + GAP) / (TILE_W + GAP));
		return Math.max(1, Math.min(Math.min(MAX_COLUMNS, fit), count));
	}

	private int left(int cols) {
		return width / 2 - (cols * TILE_W + (cols - 1) * GAP) / 2;
	}

	private void rebuild() {
		clearWidgets();
		seen = Cosmetics.version();
		List<String[]> tiles = tiles();
		int cols = columns(tiles.size());
		int left = left(cols);
		Cosmetics.Look mine = Cosmetics.myLook();
		String worn = mine == null ? null : mine.cape();
		boolean canWear = Cosmetics.signedIn() && !Cosmetics.busy();
		for (int i = 0; i < tiles.size(); i++) {
			String[] t = tiles.get(i);
			boolean active = Objects.equals(t[2], worn);
			int x = left + (i % cols) * (TILE_W + GAP);
			int y = TOP + (i / cols) * (TILE_H + GAP) + TILE_H - 20;
			String id = t[0];
			Button b = Button.builder(Component.literal(active ? "✔ " + t[1] : t[1]), btn -> Cosmetics.wearCape(id))
					.bounds(x, y, TILE_W, 20)
					.build();
			b.active = canWear && !active;
			addRenderableWidget(b);
		}
		int rows = (tiles.size() + cols - 1) / cols;
		int y = TOP + rows * (TILE_H + GAP) + 8;
		for (AbstractClientPlayer p : nearbyWithLooks()) {
			UUID id = p.getUUID();
			boolean hidden = ArcticConfig.get().hiddenPlayers.contains(id.toString());
			String label = (hidden ? "Show " : "Hide ") + p.getPlainTextName() + "'s look";
			addRenderableWidget(Button.builder(Component.literal(label), btn -> {
				if (!ArcticConfig.get().hiddenPlayers.remove(id.toString())) {
					ArcticConfig.get().hiddenPlayers.add(id.toString());
				}
				ArcticConfig.save();
				rebuild();
			}).bounds(width / 2 - 100, y, 200, 20).build());
			y += 24;
		}
		int bottom = height - 28;
		String show = ArcticConfig.get().showCosmetics ? "Looks: Shown" : "Looks: Hidden";
		addRenderableWidget(Button.builder(Component.literal(show), b -> {
			ArcticConfig.get().showCosmetics = !ArcticConfig.get().showCosmetics;
			ArcticConfig.save();
			rebuild();
		}).bounds(width / 2 - 154, bottom, 150, 20).build());
		addRenderableWidget(Button.builder(Component.literal("Done"), b -> onClose()).bounds(width / 2 + 4, bottom, 150, 20).build());
	}

	/** Other players in the world wearing an Arctic look (or hidden ones). */
	private List<AbstractClientPlayer> nearbyWithLooks() {
		List<AbstractClientPlayer> out = new ArrayList<>();
		Minecraft mc = Minecraft.getInstance();
		if (mc.level == null) {
			return out;
		}
		UUID self = mc.getUser().getProfileId();
		for (AbstractClientPlayer p : mc.level.players()) {
			boolean hidden = ArcticConfig.get().hiddenPlayers.contains(p.getUUID().toString());
			if (!p.getUUID().equals(self) && (hidden || Cosmetics.lookFor(p.getUUID()) != null)) {
				out.add(p);
				if (out.size() == MAX_NEARBY) {
					break;
				}
			}
		}
		return out;
	}

	@Override
	public void tick() {
		if (seen != Cosmetics.version()) {
			rebuild();
		}
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		super.extractRenderState(g, mouseX, mouseY, delta);
		g.centeredText(font, Component.literal("Arctic"), width / 2, 12, ICE);
		String status = Cosmetics.status().isEmpty()
				? "Pick a cape. Change your skin in Arctic Launcher."
				: Cosmetics.status();
		g.centeredText(font, Component.literal(status), width / 2, 28, MUTED);
		List<String[]> tiles = tiles();
		int cols = columns(tiles.size());
		int left = left(cols);
		for (int i = 0; i < tiles.size(); i++) {
			int x = left + (i % cols) * (TILE_W + GAP);
			int y = TOP + (i / cols) * (TILE_H + GAP);
			int h = TILE_H - 22;
			g.fill(x, y, x + TILE_W, y + h, 0x66101A2A);
			g.outline(x, y, TILE_W, h, 0x557DD3FC);
			Identifier tex = Cosmetics.texture(tiles.get(i)[2]);
			if (tex != null) {
				// Cape front: (1,1) 10x16 of a 64x32 layout; preset textures are 2x.
				int w = 26;
				int ch = 42;
				g.blit(RenderPipelines.GUI_TEXTURED, tex, x + (TILE_W - w) / 2, y + (h - ch) / 2, 2f, 2f, w, ch, 20, 32, 128, 64);
			} else if (i == 0) {
				g.centeredText(font, Component.literal("—"), x + TILE_W / 2, y + h / 2 - 4, MUTED);
			}
		}
	}

	@Override
	public void onClose() {
		minecraft.gui.setScreen(parent);
	}
}
