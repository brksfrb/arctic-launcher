package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.gui.components.Button;
import net.minecraft.network.chat.Component;

/** The "Arctic" button on the pause screen and the Vanilla title screen. */
public final class ArcticButton {
	private ArcticButton() {}

	private static final java.util.Set<Button> OWN = java.util.Collections.newSetFromMap(new java.util.WeakHashMap<Button, Boolean>());
	private static final java.util.Map<net.minecraft.client.gui.screens.Screen, Runnable> TICKS = new java.util.WeakHashMap<>();

	public static Button create() { return create("Arctic", () -> ArcticClient.platform().openPage(ArcticClient.arcticMenu())); }
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
}
