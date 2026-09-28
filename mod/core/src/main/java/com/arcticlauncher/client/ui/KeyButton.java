package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;

/**
 * Shows a key binding; click it, then press a key or a mouse button to
 * change it (Escape clears it, like Minecraft's controls). The page
 * forwards the next key press.
 */
public class KeyButton extends Widget {
	/** Reads and writes the binding (a Minecraft key name). */
	public interface Binding {
		String get();

		void set(String key);
	}

	public static final String UNBOUND = "key.keyboard.unknown";

	private final Binding binding;
	private boolean listening;

	public KeyButton(Binding binding) {
		this.binding = binding;
	}

	public boolean listening() {
		return listening;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		if (listening) {
			Skin.primary(g, s, x, y, w, h, true, 1f);
			Draw.centered(g, Draw.fitCentered(g, "Press a key (Esc: none)", w - 6, x + w / 2, y + (h - 8) / 2), x + w / 2, y + (h - 8) / 2,
					s.onAccent, false);
			return;
		}
		Skin.button(g, s, x, y, w, h, enabled, hover);
		String key = binding.get();
		String label = key == null || UNBOUND.equals(key) ? "None" : ArcticClient.platform().keyLabel(key);
		Draw.centered(g, Draw.fitCentered(g, label, w - 6, x + w / 2, y + (h - 8) / 2), x + w / 2, y + (h - 8) / 2, s.text, true);
	}

	/**
	 * Left click starts listening (and, while listening, cancels). While
	 * listening, any other mouse button becomes the binding (Mouse 4 and 5
	 * are popular for zoom).
	 */
	@Override
	public boolean click(double mx, double my, int button) {
		if (!enabled) {
			return false;
		}
		if (listening && button != Keys.MOUSE_LEFT) {
			listening = false;
			binding.set(mouseName(button));
			ArcticClient.saveConfig();
			return true;
		}
		if (button != Keys.MOUSE_LEFT) {
			return false;
		}
		listening = !listening;
		return true;
	}

	/** Minecraft's name for a mouse button (0 left, 1 right, 2 middle, then 4, 5…). */
	static String mouseName(int button) {
		switch (button) {
			case 0:
				return "key.mouse.left";
			case 1:
				return "key.mouse.right";
			case 2:
				return "key.mouse.middle";
			default:
				return "key.mouse." + (button + 1);
		}
	}

	/** Take a key press while listening; true if used. Escape (or Backspace) clears it, like Minecraft's controls. */
	public boolean keyPressed(int key, int nativeKey) {
		if (!listening) {
			return false;
		}
		listening = false;
		if (key == Keys.ESCAPE || key == Keys.BACKSPACE) {
			binding.set(UNBOUND);
		} else {
			binding.set(ArcticClient.platform().keyName(nativeKey));
		}
		ArcticClient.saveConfig();
		return true;
	}
}
