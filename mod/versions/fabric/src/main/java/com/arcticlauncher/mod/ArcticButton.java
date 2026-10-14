package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.gui.components.Button;
import net.minecraft.network.chat.Component;

/** The "Arctic" button on the pause screen and the Vanilla title screen. */
public final class ArcticButton {
	private ArcticButton() {}

	private static final java.util.Set<Button> OWN = java.util.Collections.newSetFromMap(new java.util.WeakHashMap<Button, Boolean>());
	private static final java.util.Set<Button> MAIN = java.util.Collections.newSetFromMap(new java.util.WeakHashMap<Button, Boolean>());
	private static final java.util.Map<net.minecraft.client.gui.screens.Screen, Runnable> TICKS = new java.util.WeakHashMap<>();

	public static Button create() {
		Button button = create("Arctic", () -> ArcticClient.platform().openPage(ArcticClient.arcticMenu()));
		MAIN.add(button); return button;
	}
	public static Button create(String label, Runnable action) {
		Button made;
		//#if MC >= 1.19.3
		made = Button.builder(Compat.literal(label), b -> action.run()).bounds(4, 4, 76, 20).build();
		//#elif MC >= 1.16
		made = new Button(4, 4, 76, 20, Compat.literal(label), b -> action.run());
		//#else
		made = new Button(4, 4, 76, 20, label, b -> action.run());
		//#endif
		OWN.add(made); return made;
	}

	public static void addActions(net.minecraft.client.gui.screens.Screen screen, java.util.function.Consumer<Button> add) {
		add.accept(create());
		if (screen instanceof net.minecraft.client.gui.screens.TitleScreen) {
			add.accept(create("Replays", () -> ArcticClient.platform().openPage(com.arcticlauncher.client.menu.Menus.page("replays"))));
			add.accept(create("Edit HUD", () -> ArcticClient.platform().openPage(new com.arcticlauncher.client.menu.HudEditor())));
		} else if (screen instanceof net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen) {
			add.accept(create("Arctic servers", () -> ServerMenus.open(screen)));
		} else if (screen instanceof net.minecraft.client.gui.screens.DisconnectedScreen) {
			add.accept(create("Arctic disconnect", () -> ServerMenus.open(screen)));
			com.arcticlauncher.client.servers.DisconnectedPage reconnect = new com.arcticlauncher.client.servers.DisconnectedPage("", "");
			if (reconnect.canReconnect()) {
				Button join = create("Reconnect", reconnect::reconnect); add.accept(join);
				Button cancel = create("Cancel auto", reconnect::cancelReconnect); add.accept(cancel);
				TICKS.put(screen, () -> {
					cancel.visible = reconnect.autoOn();
					//#if MC >= 1.16
					join.setMessage(Compat.literal(reconnect.autoOn() ? "Reconnect " + reconnect.secondsLeft() + "s" : "Reconnect"));
					//#else
					join.setMessage(reconnect.autoOn() ? "Reconnect " + reconnect.secondsLeft() + "s" : "Reconnect");
					//#endif
					reconnect.tickReconnect();
				});
			}
		}
	}

	/** Runs after all mods have initialized. Move only our buttons into vacant places. */
	public static void layout(net.minecraft.client.gui.screens.Screen screen) {
		Runnable tick = TICKS.get(screen); if (tick != null) tick.run();
		java.util.List<net.minecraft.client.gui.components.AbstractWidget> placed = new java.util.ArrayList<>();
		java.util.List<Button> ours = new java.util.ArrayList<>();
		for (Object child : screen.children()) {
			if (child instanceof Button && OWN.contains(child)) ours.add((Button) child);
			else if (child instanceof net.minecraft.client.gui.components.AbstractWidget && ((net.minecraft.client.gui.components.AbstractWidget) child).visible)
				placed.add((net.minecraft.client.gui.components.AbstractWidget) child);
		}
		for (Button button : ours) {
			if (!button.visible) continue;
			if (ArcticClient.restyles() && screen instanceof net.minecraft.client.gui.screens.TitleScreen
					&& MAIN.contains(button) && centerTitleAction(screen, button, placed)) {
				placed.add(button); continue;
			}
			button.setWidth(76);
			boolean found = false;
			for (int y = 4; y + 20 <= screen.height - 4 && !found; y += 24) {
				for (int x = 4; x + 76 <= screen.width - 4; x += 80) {
					boolean free = true;
					for (net.minecraft.client.gui.components.AbstractWidget other : placed) {
						int ox = Compat.widgetX(other), oy = Compat.widgetY(other);
						if (x < ox + other.getWidth() + 3 && x + 79 > ox && y < oy + Compat.widgetHeight(other) + 3 && y + 23 > oy) { free = false; break; }
					}
					if (!free) continue;
					//#if MC >= 1.19.3
					button.setX(x); button.setY(y);
					//#else
					button.x = x; button.y = y;
					//#endif
					placed.add(button); found = true; break;
				}
			}
		}
	}

	/** Resize the two vanilla actions in place; their callbacks and third-party widgets stay intact. */
	private static boolean centerTitleAction(net.minecraft.client.gui.screens.Screen screen, Button arctic,
			java.util.List<net.minecraft.client.gui.components.AbstractWidget> placed) {
		net.minecraft.client.gui.components.AbstractWidget options = null, quit = null;
		for (net.minecraft.client.gui.components.AbstractWidget widget : placed) {
			// Newer versions return Button.Plain from the same vanilla builder used above.
			if (widget.getClass() != arctic.getClass()) continue;
			//#if MC >= 1.16
			String key = Compat.translationKey(widget.getMessage());
			if ("menu.options".equals(key)) options = widget;
			if ("menu.quit".equals(key)) quit = widget;
			//#else
			String label = widget.getMessage();
			if (ArcticClient.platform().translate("menu.options", "Options...").equals(label)) options = widget;
			if (ArcticClient.platform().translate("menu.quit", "Quit Game").equals(label)) quit = widget;
			//#endif
		}
		if (options != null && quit != null && Compat.widgetY(options) == Compat.widgetY(quit)) {
			int left = Compat.widgetX(options), right = Compat.widgetX(quit) + quit.getWidth(), y = Compat.widgetY(options);
			int third = (right - left - 8) / 3;
			if (third >= 50 && free(left + third + 4, y, third, placed, options, quit)) {
				options.setWidth(third);
				bounds(arctic, left + third + 4, y, third);
				bounds(quit, left + (third + 4) * 2, y, right - left - (third + 4) * 2);
				return true;
			}
		}
		// A mod changed the vanilla row: keep its layout and put our action in a free centered row.
		for (int y = screen.height / 4 + 48; y + 20 < screen.height - 16; y += 24) {
			if (free(screen.width / 2 - 38, y, 76, placed, null, null)) {
				bounds(arctic, screen.width / 2 - 38, y, 76); return true;
			}
		}
		return false;
	}

	private static boolean free(int x, int y, int width, java.util.List<net.minecraft.client.gui.components.AbstractWidget> placed,
			net.minecraft.client.gui.components.AbstractWidget exceptA, net.minecraft.client.gui.components.AbstractWidget exceptB) {
		for (net.minecraft.client.gui.components.AbstractWidget other : placed) {
			if (other == exceptA || other == exceptB) continue;
			int ox = Compat.widgetX(other), oy = Compat.widgetY(other);
			if (x < ox + other.getWidth() + 3 && x + width + 3 > ox && y < oy + Compat.widgetHeight(other) + 3 && y + 23 > oy) return false;
		}
		return true;
	}

	private static void bounds(net.minecraft.client.gui.components.AbstractWidget widget, int x, int y, int width) {
		//#if MC >= 1.19.3
		widget.setX(x); widget.setY(y);
		//#else
		widget.x = x; widget.y = y;
		//#endif
		widget.setWidth(width);
	}
}
