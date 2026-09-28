package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ui.Page;
import net.minecraft.client.gui.screen.Screen;
import org.lwjgl.input.Keyboard;
import org.lwjgl.input.Mouse;

/** Shows a core {@link Page} as an old-Minecraft screen and forwards input. */
public final class LegacyPageScreen extends Screen {
	private final Page page;
	private final Screen parent;

	/** The screen under the Arctic menu (the game's, or none). */
	Screen parent() {
		return parent;
	}

	public LegacyPageScreen(Page page, Screen parent) {
		this.page = page;
		this.parent = parent;
	}

	public Page page() {
		return page;
	}

	@Override
	public void init() {
		Keyboard.enableRepeatEvents(true);
		page.init(width, height);
	}

	@Override
	public void tick() {
		page.tick();
	}

	@Override
	public void render(int mouseX, int mouseY, float delta) {
		if (!page.ownBackground() && page.dimWorld()) {
			renderBackground();
		}
		page.render(new LegacyGfx(), mouseX, mouseY);
	}

	@Override
	protected void mouseClicked(int x, int y, int button) {
		page.mouseClicked(x, y, LegacyKeys.coreMouse(button));
	}

	@Override
	protected void mouseReleased(int x, int y, int button) {
		page.mouseReleased(x, y, LegacyKeys.coreMouse(button));
	}

	@Override
	protected void mouseDragged(int x, int y, int button, long held) {
		page.mouseDragged(x, y, LegacyKeys.coreMouse(button));
	}

	@Override
	public void handleMouse() {
		super.handleMouse();
		int wheel = Mouse.getEventDWheel();
		if (wheel != 0) {
			int x = Mouse.getEventX() * width / client.width;
			int y = height - Mouse.getEventY() * height / client.height - 1;
			page.mouseScrolled(x, y, wheel > 0 ? 1 : -1);
		}
	}

	@Override
	protected void keyPressed(char c, int code) {
		boolean used = page.keyPressed(LegacyKeys.core(code), code);
		if (!used && c >= ' ' && c != 127) {
			used = page.charTyped(c);
		}
		if (!used && code == Keyboard.KEY_ESCAPE && page.closeOnEscape()) {
			close();
		}
	}

	@Override
	public void removed() {
		Keyboard.enableRepeatEvents(false);
		page.removed();
	}

	@Override
	public boolean shouldPauseGame() {
		return page.pausesGame();
	}

	/** Back to where the page was opened from. */
	public void close() {
		client.setScreen(parent);
	}
}
