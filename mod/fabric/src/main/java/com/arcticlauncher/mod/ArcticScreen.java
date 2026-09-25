package com.arcticlauncher.mod;

import java.util.List;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;

/** The in-game Arctic menu: pick a cape, toggle cosmetics. */
public final class ArcticScreen extends Screen {
	private static final int MAX_COLUMNS = 5;
	private static final int TILE_W = 74;
	private static final int TILE_H = 96;
	private static final int GAP = 8;
	private static final int ICE = 0xFF7DD3FC;
	private static final int MUTED = 0xFFA9BCD0;

	private final Screen parent;
	private int seen = -1;

	public ArcticScreen(Screen parent) {
		super(Component.literal("Arctic"));
		this.parent = parent;
	}

	/** Button that opens the menu, for the title and pause screens. */
	public static Button openButton(Screen from) {
		return Button.builder(Component.literal("Arctic"), b -> net.minecraft.client.Minecraft.getInstance().gui.setScreen(new ArcticScreen(from)))
				.bounds(4, 4, 64, 20)
				.build();
	}

	@Override
	protected void init() {
		if (!Cosmetics.signedIn() && !Cosmetics.busy()) {
			Cosmetics.signIn();
		}
		rebuild();
	}

	private void rebuild() {
		clearWidgets();
		seen = Cosmetics.version();
		List<Cosmetics.Item> items = Cosmetics.catalog();
		int count = items.size() + 1;
		int cols = columns(count);
		int left = width / 2 - (cols * TILE_W + (cols - 1) * GAP) / 2;
		int top = 58;
		boolean canEquip = Cosmetics.signedIn() && !Cosmetics.busy();
		for (int i = 0; i < count; i++) {
			String id = i == 0 ? null : items.get(i - 1).id();
			String name = i == 0 ? "No cape" : items.get(i - 1).name();
			boolean active = java.util.Objects.equals(id, Cosmetics.equipped()) && Cosmetics.signedIn();
			int x = left + (i % cols) * (TILE_W + GAP);
			int y = top + (i / cols) * (TILE_H + GAP) + TILE_H - 20;
			Button button = Button.builder(Component.literal(active ? "✔ " + name : name), b -> Cosmetics.equip(id))
					.bounds(x, y, TILE_W, 20)
					.build();
			button.active = canEquip && !active;
			addRenderableWidget(button);
		}
		int bottom = height - 28;
		String show = ArcticConfig.get().showCosmetics ? "Cosmetics: Shown" : "Cosmetics: Hidden";
		addRenderableWidget(Button.builder(Component.literal(show), b -> {
			ArcticConfig.get().showCosmetics = !ArcticConfig.get().showCosmetics;
			ArcticConfig.save();
			rebuild();
		}).bounds(width / 2 - 154, bottom, 150, 20).build());
		addRenderableWidget(Button.builder(Component.literal("Done"), b -> onClose()).bounds(width / 2 + 4, bottom, 150, 20).build());
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
		g.centeredText(font, Component.literal("Arctic"), width / 2, 16, ICE);
		g.centeredText(font, Component.literal(Cosmetics.status()), width / 2, 32, MUTED);
		List<Cosmetics.Item> items = Cosmetics.catalog();
		int count = items.size() + 1;
		int cols = columns(count);
		int left = width / 2 - (cols * TILE_W + (cols - 1) * GAP) / 2;
		for (int i = 0; i < count; i++) {
			int x = left + (i % cols) * (TILE_W + GAP);
			int y = 58 + (i / cols) * (TILE_H + GAP);
			g.fill(x, y, x + TILE_W, y + TILE_H - 22, 0x66101A2A);
			g.outline(x, y, TILE_W, TILE_H - 22, 0x557DD3FC);
			if (i > 0) {
				Identifier tex = Cosmetics.texture(items.get(i - 1).id());
				if (tex != null) {
					// Cape front: (1,1) 10x16 of a 64x32 layout, textures are 2x.
					int w = 30;
					int h = 48;
					g.blit(RenderPipelines.GUI_TEXTURED, tex, x + (TILE_W - w) / 2, y + (TILE_H - 22 - h) / 2, 2f, 2f, w, h, 20, 32, 128, 64);
				}
			} else {
				g.centeredText(font, Component.literal("—"), x + TILE_W / 2, y + (TILE_H - 22) / 2 - 4, MUTED);
			}
		}
	}

	/** Tiles per row: as many as fit, up to MAX_COLUMNS. */
	private int columns(int count) {
		int fit = Math.max(1, (width - 20 + GAP) / (TILE_W + GAP));
		return Math.max(1, Math.min(Math.min(MAX_COLUMNS, fit), count));
	}

	@Override
	public void onClose() {
		minecraft.gui.setScreen(parent);
	}
}
