package com.arcticlauncher.mod;

//#if MC >= 1.14
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.client.gui.screens.inventory.CreativeModeInventoryScreen;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.ContainerInput;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.item.ItemStack;
import java.util.ArrayList;
import java.util.List;
//#endif

/** Client Sort's view of the open chest or inventory: which slots, what is in them, and the clicks. */
public final class ContainerSort {
	private ContainerSort() {}

	//#if MC >= 1.14
	/** The slot the mouse is over (set while the screen draws), or null. */
	public static Slot hovered;

	/** The open chest or inventory (not the creative one, whose slots aren't the game's), or null. */
	private static AbstractContainerMenu menu() {
		Screen screen = Compat.screen();
		if (!(screen instanceof AbstractContainerScreen) || screen instanceof CreativeModeInventoryScreen) {
			return null;
		}
		return ((AbstractContainerScreen<?>) screen).getMenu();
	}

	public static boolean screenOpen() {
		return menu() != null;
	}

	public static int[] slots() {
		AbstractContainerMenu menu = menu();
		Slot focus = hovered;
		if (menu == null || focus == null) {
			return null;
		}
		boolean own = focus.container instanceof Inventory;
		if (own) {
			int at = InventorySlots.index(focus);
			if (at < 9 || at > 35) {
				// The hotbar and armor keep their places.
				return null;
			}
		}
		List<Integer> picked = new ArrayList<>();
		for (int i = 0; i < menu.slots.size(); i++) {
			Slot s = menu.slots.get(i);
			if (own) {
				int at = InventorySlots.index(s);
				if (at >= 9 && at <= 35) {
					picked.add(i);
				}
			} else if (s.container == focus.container) {
				picked.add(i);
			}
		}
		int[] out = new int[picked.size()];
		for (int i = 0; i < out.length; i++) {
			out[i] = picked.get(i);
		}
		return out;
	}

	public static Object[] info(int slot) {
		AbstractContainerMenu menu = menu();
		if (menu == null || slot < 0 || slot >= menu.slots.size()) {
			return null;
		}
		ItemStack stack = menu.slots.get(slot).getItem();
		if (stack.isEmpty()) {
			return null;
		}
		String name = stack.getHoverName().getString();
		String key = System.identityHashCode(stack.getItem()) + ":" + stack.getDamageValue() + ":" + name;
		return new Object[] {key, name, stack.getCount(), stack.getMaxStackSize()};
	}

	public static void click(int slot) {
		Minecraft mc = Minecraft.getInstance();
		AbstractContainerMenu menu = menu();
		if (menu != null && mc.gameMode != null && mc.player != null) {
			mc.gameMode.handleContainerInput(menu.containerId, slot, 0, ContainerInput.PICKUP, mc.player);
		}
	}

	public static boolean cursorHolds() {
		Minecraft mc = Minecraft.getInstance();
		if (mc.player == null) {
			return false;
		}
		//#if MC >= 1.17
		return !mc.player.containerMenu.getCarried().isEmpty();
		//#else
		return !Compat.inventory(mc.player).getCarried().isEmpty();
		//#endif
	}
	//#else
	public static boolean screenOpen() {
		return false;
	}

	public static int[] slots() {
		return null;
	}

	public static Object[] info(int slot) {
		return null;
	}

	public static void click(int slot) {}

	public static boolean cursorHolds() {
		return false;
	}
	//#endif
}
