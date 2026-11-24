package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.NativeMenuButton;
import com.arcticlauncher.client.style.Style;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.components.Button;
import net.minecraft.network.chat.Component;

import com.arcticlauncher.client.Lang;
/** The "Arctic" button on the pause screen and the Vanilla title screen. */
public final class ArcticButton {
	private ArcticButton() {}

	private static final java.util.Set<Button> OWN = java.util.Collections.newSetFromMap(new java.util.WeakHashMap<Button, Boolean>());
	private static final java.util.Set<Button> MAIN = java.util.Collections.newSetFromMap(new java.util.WeakHashMap<Button, Boolean>());
	/** The numbers of Arctic's buttons on 1.13, clear of the game's own. */
	private static final java.util.Map<Button, Integer> TITLE_CORNERS = new java.util.WeakHashMap<>();
	private static final java.util.Map<Screen, Long> LAYOUTS = new java.util.WeakHashMap<>();
	private static final int OWN_IDS = 9500;
	private static final java.util.Map<net.minecraft.client.gui.screens.Screen, Runnable> TICKS = new java.util.WeakHashMap<>();

	public static boolean owns(Object button) {
		return OWN.contains(button);
	}

	public static Button create() {
		Button button = create(ArcticClient.hubLabel(), () -> ArcticClient.platform().openPage(ArcticClient.hub()));
		MAIN.add(button); return button;
	}
	public static Button create(String label, Runnable action) {
		Button made;
		//#if MC >= 1.19.3
		made = Button.builder(Compat.literal(label), b -> action.run()).bounds(4, 4, 76, 20).build();
		//#elif MC >= 1.16
		made = new Button(4, 4, 76, 20, Compat.literal(label), b -> action.run());
		//#elif MC >= 1.14
		made = new Button(4, 4, 76, 20, label, b -> action.run());
		//#else
		// 1.13: buttons are numbered, and a click is handled by the button itself (method_18374).
		made = new Button(OWN_IDS + OWN.size(), 4, 4, 76, 20, label) {
			@Override
			public void method_18374(double x, double y) {
				action.run();
			}
		};
		//#endif
		OWN.add(made); return made;
	}

	public static void addActions(net.minecraft.client.gui.screens.Screen screen, java.util.function.Consumer<Button> add) {
		add.accept(create());
		if (screen.getClass() == TitleScreen.class && ArcticClient.style() == Style.ARCTIC) {
			if (ArcticClient.platform().hasFeatures()) {
				Button hud = create(Lang.t("mods.edit_hud"), () -> ArcticClient.platform().openPage(new com.arcticlauncher.client.menu.HudEditor()));
				TITLE_CORNERS.put(hud, 3);
				add.accept(hud);
			}
			if (com.arcticlauncher.client.replay.Replays.canWatch()) {
				Button replays = create(Lang.t("mods.replays"), () -> com.arcticlauncher.client.menu.Menus.open("replays"));
				TITLE_CORNERS.put(replays, 2);
				add.accept(replays);
			}
		}
		if (screen instanceof net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen) {
			add.accept(create(Lang.t("platform.arctic_servers"), () -> ServerMenus.open(screen)));
		} else if (screen instanceof net.minecraft.client.gui.screens.DisconnectedScreen) {
			add.accept(create(Lang.t("platform.arctic_disconnect"), () -> ServerMenus.open(screen)));
			com.arcticlauncher.client.servers.DisconnectedPage reconnect = new com.arcticlauncher.client.servers.DisconnectedPage("", "");
			if (reconnect.canReconnect()) {
				Button join = create(Lang.t("platform.reconnect"), reconnect::reconnect); add.accept(join);
				Button cancel = create(Lang.t("platform.cancel_auto"), reconnect::cancelReconnect); add.accept(cancel);
				TICKS.put(screen, () -> {
					cancel.visible = reconnect.autoOn();
					//#if MC >= 1.16
					join.setMessage(Compat.literal(reconnect.autoOn() ? Lang.t("platform.reconnect_2") + reconnect.countdown() : Lang.t("platform.reconnect")));
					//#else
					Compat.setRawLabel(join, reconnect.autoOn() ? Lang.t("platform.reconnect_2") + reconnect.countdown() : Lang.t("platform.reconnect"));
					//#endif
					reconnect.tickReconnect();
				});
			}
		}
	}

	/** Place Arctic controls after the screen and other mods have initialized. */
	public static void layout(net.minecraft.client.gui.screens.Screen screen) {
		Runnable tick = TICKS.get(screen); if (tick != null) tick.run();
		java.util.List<?> children = Compat.widgets(screen);
		long stamp = layoutStamp(screen, children);
		Long previous = LAYOUTS.get(screen);
		if (previous != null && stamp == previous.longValue()) return;
		boolean foundAction = false;
		for (int i = 0; i < children.size(); i++) {
			if (owns(children.get(i))) {
				foundAction = true;
				break;
			}
		}
		if (!foundAction) {
			LAYOUTS.put(screen, stamp);
			return;
		}
		java.util.List<AbstractWidget> placed = new java.util.ArrayList<>();
		java.util.List<Button> ours = new java.util.ArrayList<>();
		for (Object child : children) {
			if (child instanceof Button && owns(child)) ours.add((Button) child);
			else if (child instanceof AbstractWidget && ((AbstractWidget) child).visible) placed.add((AbstractWidget) child);
		}
		if (screen.getClass() == TitleScreen.class && ArcticClient.style() == Style.ARCTIC) {
			titleCorners(screen, placed);
		}
		for (Button button : ours) {
			if (!button.visible) continue;
			Integer corner = TITLE_CORNERS.get(button);
			if (corner != null && titleCorner(screen, button, corner, placed)) {
				placed.add(button);
				continue;
			}
			if (ArcticClient.restyles() && screen instanceof net.minecraft.client.gui.screens.TitleScreen
					&& MAIN.contains(button) && centerTitleAction(screen, button, placed)) {
				placed.add(button); continue;
			}
			if (ArcticClient.style() == Style.ARCTIC
					&& screen.getClass() == PauseScreen.class
					&& MAIN.contains(button) && centerPauseAction(screen, button, placed)) {
				placed.add(button);
				continue;
			}
			button.setWidth(76);
			boolean found = false;
			for (int y = 4; y + 20 <= screen.height - 4 && !found; y += 24) {
				for (int x = 4; x + 76 <= screen.width - 4; x += 80) {
					boolean free = true;
					for (net.minecraft.client.gui.components.AbstractWidget other : placed) {
						int ox = Compat.widgetX(other), oy = Compat.widgetY(other);
						if (x < ox + Compat.widgetWidth(other) + 3 && x + 79 > ox && y < oy + Compat.widgetHeight(other) + 3 && y + 23 > oy) { free = false; break; }
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
		LAYOUTS.put(screen, layoutStamp(screen, children));
	}

	private static long layoutStamp(Screen screen, java.util.List<?> children) {
		long stamp = ((long) screen.width << 32) ^ screen.height ^ ArcticClient.style().id.hashCode();
		for (int i = 0; i < children.size(); i++) {
			Object child = children.get(i);
			if (!(child instanceof AbstractWidget)) continue;
			AbstractWidget widget = (AbstractWidget) child;
			stamp = stamp * 31 + System.identityHashCode(widget);
			stamp = stamp * 31 + Compat.widgetX(widget);
			stamp = stamp * 31 + Compat.widgetY(widget);
			stamp = stamp * 31 + Compat.widgetWidth(widget);
			stamp = stamp * 31 + (widget.visible ? 1 : 0);
		}
		return stamp;
	}

	private static void titleCorners(Screen screen, java.util.List<AbstractWidget> placed) {
		//#if MC >= 1.20.2
		for (AbstractWidget widget : placed) {
			if (!(widget instanceof net.minecraft.client.gui.components.SpriteIconButton)) continue;
			String key = Compat.labelKey(widget, NativeMenuButton.LABELS);
			if ("options.language".equals(key)) titleCorner(screen, widget, 1, placed);
			if ("options.accessibility".equals(key)) titleCorner(screen, widget, 0, placed);
		}
		//#endif
	}

	private static boolean titleCorner(Screen screen, AbstractWidget widget, int slot, java.util.List<AbstractWidget> placed) {
		int x = screen.width - 6 - 76 - 80 * slot;
		if (x < 4 || !free(x, 6, 76, placed, widget, null)) return false;
		bounds(widget, x, 6, 76);
		//#if MC >= 1.20.2
		widget.setHeight(16);
		//#endif
		return true;
	}

	/** Fit Arctic into the native options row, keeping both original actions. */
	private static boolean centerPauseAction(Screen screen, Button arctic, java.util.List<AbstractWidget> placed) {
		net.minecraft.client.gui.components.AbstractWidget options = null, adjacent = null;
		for (net.minecraft.client.gui.components.AbstractWidget widget : placed) {
			if (widget.getClass() != arctic.getClass()) continue;
			String key = Compat.labelKey(widget, NativeMenuButton.LABELS);
			if ("menu.options".equals(key)) options = widget;
			if ("menu.shareToLan".equals(key) || "options.worldOptions.button".equals(key)) adjacent = widget;
		}
		if (options == null) return false;
		int x = Compat.widgetX(options), y = Compat.widgetY(options);
		if (adjacent == null || Compat.widgetY(adjacent) != y) {
			int w = Compat.widgetWidth(options);
			if (!free(x + w + 4, y, w, placed, options, null)) return false;
			bounds(arctic, x + w + 4, y, w);
			return true;
		}
		int right = Compat.widgetX(adjacent) + Compat.widgetWidth(adjacent);
		//#if MC >= 26.1
		if (movePauseIcons(screen, y, placed)) {
			int leftWidth = (right - x - 4) / 2;
			bounds(options, x, y - 24, leftWidth);
			bounds(arctic, x + leftWidth + 4, y - 24, right - x - leftWidth - 4);
			bounds(adjacent, x, y, right - x);
			return true;
		}
		//#endif
		int third = (right - x - 8) / 3;
		if (third < 50 || !free(x + third + 4, y, third, placed, options, adjacent)) return false;
		options.setWidth(third);
		bounds(arctic, x + third + 4, y, third);
		bounds(adjacent, x + (third + 4) * 2, y, right - x - (third + 4) * 2);
		return true;
	}

	//#if MC >= 26.1
	private static boolean movePauseIcons(Screen screen, int optionsY, java.util.List<AbstractWidget> placed) {
		int iconY = optionsY - 24;
		int newY = optionsY + 48;
		if (newY + 22 >= screen.height) return false;
		boolean found = false;
		for (AbstractWidget widget : placed) {
			if (widget.getY() != iconY) continue;
			if (!(widget instanceof net.minecraft.client.gui.components.SpriteIconButton)
					|| !free(widget.getX(), newY, widget.getWidth(), placed, widget, null)) return false;
			found = true;
		}
		if (!found) return false;
		for (AbstractWidget widget : placed) {
			if (widget.getY() == iconY) widget.setY(newY);
		}
		return true;
	}
	//#endif

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
			String label = Compat.rawLabel(widget);
			if (ArcticClient.platform().translate("menu.options", "Options...").equals(label)) options = widget;
			if (ArcticClient.platform().translate("menu.quit", "Quit Game").equals(label)) quit = widget;
			//#endif
		}
		if (options != null && quit != null && Compat.widgetY(options) == Compat.widgetY(quit)) {
			int left = Compat.widgetX(options), right = Compat.widgetX(quit) + Compat.widgetWidth(quit), y = Compat.widgetY(options);
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
			if (x < ox + Compat.widgetWidth(other) + 3 && x + width + 3 > ox && y < oy + Compat.widgetHeight(other) + 3 && y + 23 > oy) return false;
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
