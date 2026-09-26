package com.arcticlauncher.mod;

import com.arcticlauncher.client.ui.Page;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

/** Shows a core {@link Page} as a Minecraft screen and forwards input. */
public final class PageScreen extends Screen {
	private final Page page;
	private final Screen parent;

	public PageScreen(Page page, Screen parent) {
		super(Component.literal("Arctic"));
		this.page = page;
		this.parent = parent;
	}

	public Page page() {
		return page;
	}

	@Override
	protected void init() {
		page.init(width, height);
	}

	@Override
	public void tick() {
		page.tick();
	}

	@Override
	public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		// Pages with their own backdrop draw it with the rest.
		if (!page.ownBackground() && page.dimWorld()) {
			super.extractBackground(g, mouseX, mouseY, delta);
		}
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		page.render(new GfxImpl(g), mouseX, mouseY);
	}

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		return page.mouseClicked(event.x(), event.y(), Input.mouse(event.button()));
	}

	@Override
	public boolean mouseReleased(MouseButtonEvent event) {
		return page.mouseReleased(event.x(), event.y(), Input.mouse(event.button()));
	}

	@Override
	public boolean mouseDragged(MouseButtonEvent event, double dx, double dy) {
		return page.mouseDragged(event.x(), event.y(), Input.mouse(event.button()));
	}

	@Override
	public boolean mouseScrolled(double x, double y, double scrollX, double scrollY) {
		return page.mouseScrolled(x, y, scrollY);
	}

	@Override
	public boolean keyPressed(KeyEvent event) {
		return page.keyPressed(Input.key(event.key()));
	}

	@Override
	public boolean shouldCloseOnEsc() {
		return page.closeOnEscape();
	}

	@Override
	public boolean isPauseScreen() {
		return page.pausesGame();
	}

	@Override
	public void onClose() {
		minecraft.gui.setScreen(parent);
	}
}
