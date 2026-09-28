package com.arcticlauncher.mod;

import java.lang.ref.WeakReference;
import java.util.Set;
import net.minecraft.ChatFormatting;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.screens.PauseScreen;

/**
 * The pause menu's Disconnect (or Save and Quit) button asks first: the
 * first click turns it red, a second click within a few seconds leaves.
 */
public final class DisconnectConfirm {
	private static final long CONFIRM_MS = 3000;
	private static final String DISCONNECT = "menu.disconnect";
	private static final String SAVE_AND_QUIT = "menu.returnToMenu";
	private static final Set<String> KEYS = new java.util.HashSet<String>(java.util.Arrays.asList(DISCONNECT, SAVE_AND_QUIT));

	private static WeakReference<AbstractWidget> armed = new WeakReference<>(null);
	private static Object original;
	private static long until;

	private DisconnectConfirm() {}

	/** Whether this press should be swallowed (it was the first click). */
	public static boolean intercept(AbstractWidget button) {
		if (!(Compat.screen() instanceof PauseScreen) || !com.arcticlauncher.client.ArcticClient.config().confirmLeave) {
			return false;
		}
		if (isArmed(button)) {
			disarm();
			return false;
		}
		String key = Compat.labelKey(button, KEYS);
		if (key == null || !KEYS.contains(key)) {
			return false;
		}
		disarm();
		original = Compat.label(button);
		String prompt = DISCONNECT.equals(key) ? "Click again to disconnect" : "Click again to save and quit";
		Compat.setLabel(button, com.arcticlauncher.mod.Compat.literal(prompt).withStyle(ChatFormatting.RED));
		armed = new WeakReference<>(button);
		until = System.currentTimeMillis() + CONFIRM_MS;
		return true;
	}

	public static boolean isArmed(AbstractWidget button) {
		return button != null && armed.get() == button && System.currentTimeMillis() < until;
	}

	/** Each tick: put the label back once the confirm window passes. */
	public static void tick() {
		if (armed.get() != null && System.currentTimeMillis() >= until) {
			disarm();
		}
	}

	private static void disarm() {
		AbstractWidget button = armed.get();
		if (button != null && original != null) {
			Compat.setLabel(button, original);
		}
		armed = new WeakReference<>(null);
		original = null;
	}
}
