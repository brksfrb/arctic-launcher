package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;

/**
 * Shows a key binding; click it, then press a key to change it (Escape
 * cancels, Backspace clears). The page forwards the next key press.
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
			Draw.centered(g, "Press a key", x + w / 2, y + (h - 8) / 2, s.onAccent, false);
			return;
		}
		Skin.button(g, s, x, y, w, h, enabled, hover);
		String key = binding.get();
		String label = key == null || UNBOUND.equals(key) ? "None" : ArcticClient.platform().keyLabel(key);
		Draw.centered(g, Draw.fit(g, label, w - 6), x + w / 2, y + (h - 8) / 2, s.text, true);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (!enabled || button != Keys.MOUSE_LEFT) {
			return false;
		}
		listening = !listening;
		return true;
	}

	/** Take a key press while listening; true if used. */
	public boolean keyPressed(int key, int nativeKey) {
		if (!listening) {
			return false;
		}
		listening = false;
		if (key == Keys.BACKSPACE) {
			binding.set(UNBOUND);
		} else if (key != Keys.ESCAPE) {
			binding.set(ArcticClient.platform().keyName(nativeKey));
		}
		ArcticClient.saveConfig();
		return true;
	}
}
