//#if MC >= 1.14
package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.SlotLocks;
import java.lang.reflect.Method;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.item.ItemStack;

/**
 * Self-test for Lock Slots ({@code -Darctic.selftest=slotlock}): a fresh world, items in the hotbar, the
 * inventory screen open, one slot locked. A click that would pick the locked item up must do nothing and
 * the same click on an unlocked slot must work; the drop key must keep the locked item. The log says
 * what happened and a screenshot shows the padlock.
 */
final class SlotLockTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-slotlocktest");
		t.setDaemon(true);
		return t;
	});
	private static final int STEP_SECONDS = 4;

	private SlotLockTest() {}

	static boolean requested() {
		return "slotlock".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticMod.LOG.info("slotlocktest: scheduled");
		TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(SlotLockTest::steps)), 10, TimeUnit.SECONDS);
	}

	private static void run(Runnable r) {
		Minecraft.getInstance().execute(() -> {
			if (Compat.screen() instanceof net.minecraft.client.gui.screens.PauseScreen) {
				Compat.setScreen(null);
			}
			r.run();
		});
	}

	private static void command(String command) {
		Compat.sendChat("/" + command);
	}

	private static void steps() {
		ClientConfig c = ArcticClient.config();
		Runnable[] steps = {
				() -> {
					c.slotLocks = true;
					command("gamemode survival");
					command("tp @s ~ 200 ~");
					command("clear @s");
					command("give @s minecraft:diamond 5");
					command("give @s minecraft:apple 3");
				},
				() -> {
					Minecraft mc = Minecraft.getInstance();
					Compat.setScreen(new net.minecraft.client.gui.screens.inventory.InventoryScreen(mc.player));
					SlotLocks.toggle(c, 0);
					ArcticMod.LOG.info("slotlocktest: locked slots {}", java.util.Arrays.toString(c.lockedSlots));
				},
				SlotLockTest::check,
				() -> {
					WorldTest.shot("slotlock-inventory");
				},
				() -> {
					c.slotLocks = false;
					c.lockedSlots = new int[0];
					ArcticMod.LOG.info("slotlocktest: done");
					Minecraft.getInstance().stop();
				},
		};
		for (int i = 0; i < steps.length; i++) {
			Runnable step = steps[i];
			TIMER.schedule(() -> run(step), (long) i * STEP_SECONDS, TimeUnit.SECONDS);
		}
	}

	private static void check() {
		Minecraft mc = Minecraft.getInstance();
		net.minecraft.client.gui.screens.Screen screen = Compat.screen();
		if (!(screen instanceof net.minecraft.client.gui.screens.inventory.AbstractContainerScreen)) {
			ArcticMod.LOG.error("slotlocktest: FAILED no container screen open");
			return;
		}
		net.minecraft.world.inventory.AbstractContainerMenu menu =
				((net.minecraft.client.gui.screens.inventory.AbstractContainerScreen<?>) screen).getMenu();
		Slot locked = slot(menu, 0);
		Slot open = slot(menu, 1);
		ArcticMod.LOG.info("slotlocktest: slot 0 holds {} x{}, slot 1 holds {} x{}", locked.getItem().getItem(), locked.getItem().getCount(),
				open.getItem().getItem(), open.getItem().getCount());
		click(screen, locked);
		ArcticMod.LOG.info("slotlocktest: pick up from the locked slot: cursor empty = {} (expected true)", carried(menu).isEmpty());
		click(screen, open);
		ArcticMod.LOG.info("slotlocktest: pick up from an unlocked slot: cursor empty = {} (expected false)", carried(menu).isEmpty());
		click(screen, open);
		Compat.setScreen(null);
		Compat.selectSlot(mc.player, 0);
		int before = Compat.inventory(mc.player).getItem(0).getCount();
		drop(mc);
		ArcticMod.LOG.info("slotlocktest: drop with the locked slot in hand: {} -> {} (expected unchanged)", before,
				Compat.inventory(mc.player).getItem(0).getCount());
		Compat.selectSlot(mc.player, 1);
		int beforeOpen = Compat.inventory(mc.player).getItem(1).getCount();
		drop(mc);
		ArcticMod.LOG.info("slotlocktest: drop with an unlocked slot in hand: {} -> {} (expected one less)", beforeOpen,
				Compat.inventory(mc.player).getItem(1).getCount());
		Compat.setScreen(new net.minecraft.client.gui.screens.inventory.InventoryScreen(mc.player));
	}

	private static ItemStack carried(net.minecraft.world.inventory.AbstractContainerMenu menu) {
		//#if MC >= 1.17
		return menu.getCarried();
		//#else
		return Minecraft.getInstance().player.inventory.getCarried();
		//#endif
	}

	private static Slot slot(net.minecraft.world.inventory.AbstractContainerMenu menu, int inventoryIndex) {
		for (Slot s : menu.slots) {
			if (InventorySlots.index(s) == inventoryIndex) {
				return s;
			}
		}
		throw new IllegalStateException("no inventory slot " + inventoryIndex);
	}

	/** A left click on the slot, through the screen's own click method (found by its shape, so any mapping works). */
	private static void click(net.minecraft.client.gui.screens.Screen screen, Slot slot) {
		for (Method m : net.minecraft.client.gui.screens.inventory.AbstractContainerScreen.class.getDeclaredMethods()) {
			Class<?>[] p = m.getParameterTypes();
			if (p.length == 4 && p[0] == Slot.class && p[1] == int.class && p[2] == int.class && p[3].isEnum()
					&& java.lang.reflect.Modifier.isProtected(m.getModifiers())) {
				try {
					m.setAccessible(true);
					Object pickup = p[3].getEnumConstants()[0];
					m.invoke(screen, slot, slot.index, 0, pickup);
					return;
				} catch (ReflectiveOperationException e) {
					throw new IllegalStateException(e);
				}
			}
		}
		throw new IllegalStateException("no slotClicked");
	}

	private static void drop(Minecraft mc) {
		//#if MC >= 26.3
		mc.gameMode.dropItem(mc.player, false);
		//#else
		mc.player.drop(false);
		//#endif
	}
}
//#endif
