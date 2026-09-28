package com.arcticlauncher.mod;

import com.arcticlauncher.client.ui.Page;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
//#if MC >= 1.21.9
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
//#endif
import net.minecraft.network.chat.Component;

/** Shows a core {@link Page} as a Minecraft screen and forwards input. */
public final class PageScreen extends Screen {
	private final Page page;
	private final Screen parent;

	/** The screen under the Arctic menu (the game's, or none). */
	public Screen parent() {
		return parent;
	}

	public PageScreen(Page page, Screen parent) {
		super(com.arcticlauncher.mod.Compat.literal("Arctic"));
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

	//#if MC >= 1.20.2
	@Override
	public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		// Pages with their own backdrop draw it with the rest.
		if (!page.ownBackground() && page.dimWorld()) {
			super.extractBackground(g, mouseX, mouseY, delta);
		}
	}
	//#elif MC >= 1.20
	// Before 1.20.2 renderBackground took no mouse position or partial tick.
	@Override
	public void extractBackground(GuiGraphicsExtractor g) {
		if (!page.ownBackground() && page.dimWorld()) {
			super.extractBackground(g);
		}
	}
	//#elif MC >= 1.16
	@Override
	public void renderBackground(com.mojang.blaze3d.vertex.PoseStack pose) {
		if (!page.ownBackground() && page.dimWorld()) {
			super.renderBackground(pose);
		}
	}
	//#else
	@Override
	public void renderBackground() {
		if (!page.ownBackground() && page.dimWorld()) {
			super.renderBackground();
		}
	}
	//#endif

	//#if MC >= 1.20
	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		page.render(new GfxImpl(g), mouseX, mouseY);
	}
	//#elif MC >= 1.16
	@Override
	public void render(com.mojang.blaze3d.vertex.PoseStack pose, int mouseX, int mouseY, float delta) {
		page.render(GfxImpl.of(pose), mouseX, mouseY);
	}
	//#else
	// Before 1.16 screens drew with no PoseStack (the global matrix).
	@Override
	public void render(int mouseX, int mouseY, float delta) {
		page.render(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), mouseX, mouseY);
	}
	//#endif

	//#if MC >= 1.21.9
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
		return page.keyPressed(Input.key(event.key()), event.key());
	}

	@Override
	public boolean charTyped(CharacterEvent event) {
		return page.charTyped(event.codepoint());
	}
	//#else
	@Override
	public boolean mouseClicked(double x, double y, int button) {
		return page.mouseClicked(x, y, Input.mouse(button));
	}

	@Override
	public boolean mouseReleased(double x, double y, int button) {
		return page.mouseReleased(x, y, Input.mouse(button));
	}

	@Override
	public boolean mouseDragged(double x, double y, int button, double dx, double dy) {
		return page.mouseDragged(x, y, Input.mouse(button));
	}

	//#if MC >= 1.20.2
	@Override
	public boolean mouseScrolled(double x, double y, double scrollX, double scrollY) {
		return page.mouseScrolled(x, y, scrollY);
	}
	//#else
	// Before 1.20.2 scrolling had no separate horizontal axis.
	@Override
	public boolean mouseScrolled(double x, double y, double scrollY) {
		return page.mouseScrolled(x, y, scrollY);
	}
	//#endif

	@Override
	public boolean keyPressed(int key, int scancode, int modifiers) {
		return page.keyPressed(Input.key(key), key);
	}

	@Override
	public boolean charTyped(char c, int modifiers) {
		return page.charTyped(c);
	}
	//#endif

	@Override
	public void removed() {
		page.removed();
		super.removed();
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
		Compat.setScreen(parent);
	}
}
