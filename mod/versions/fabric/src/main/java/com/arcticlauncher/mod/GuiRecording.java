//#if MC >= 26.1
package com.arcticlauncher.mod;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.renderer.state.gui.GuiElementRenderState;
import net.minecraft.client.renderer.state.gui.GuiItemRenderState;
import net.minecraft.client.renderer.state.gui.GuiRenderState;
import net.minecraft.client.renderer.state.gui.GuiTextRenderState;
import net.minecraft.client.renderer.state.gui.pip.PictureInPictureRenderState;

/**
 * GUI pieces as the game collects them for a frame, kept to hand in again
 * on later frames (see Gfx#startRecording): a HUD widget showing the same
 * thing reuses them, already measured and with its text already shaped.
 * Render thread only.
 */
public final class GuiRecording {
	private static List<Object> recording;
	private static int fontGeneration;

	private static final class Recording extends ArrayList<Object> {
		final int fonts = fontGeneration;
	}

	/** A resource reload closed the recorded glyph textures. Refuse them before submitting any pieces. */
	public static void fontsReloaded() {
		fontGeneration++;
		GfxImpl.clearTextWidths();
	}

	private GuiRecording() {}

	/** A piece the game was handed (GuiRenderStateRecordMixin). */
	public static void piece(Object state) {
		if (recording != null) {
			recording.add(state);
		}
	}

	static boolean start() {
		if (recording != null) {
			return false;
		}
		recording = new Recording();
		return true;
	}

	static List<Object> stop() {
		List<Object> pieces = recording;
		recording = null;
		return pieces;
	}

	static boolean replay(GuiRenderState state, List<?> pieces) {
		if (!(pieces instanceof Recording) || ((Recording) pieces).fonts != fontGeneration) {
			return false;
		}
		for (Object piece : pieces) {
			if (piece instanceof GuiItemRenderState item) {
				state.addItem(item);
			} else if (piece instanceof GuiTextRenderState text) {
				state.addText(text);
			} else if (piece instanceof PictureInPictureRenderState picture) {
				state.addPicturesInPictureState(picture);
			} else if (piece instanceof GuiElementRenderState element) {
				state.addGuiElement(element);
			}
		}
		return true;
	}
}
//#endif
