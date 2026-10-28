package com.arcticlauncher.mod;

//#if MC >= 1.16
import net.minecraft.client.Minecraft;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.ai.attributes.Attributes;
//#if MC >= 1.21.11
import net.minecraft.world.entity.animal.equine.AbstractHorse;
//#else
import net.minecraft.world.entity.animal.horse.AbstractHorse;
//#endif
//#endif

/** Horse Stats: speed and jump height of the horse you ride or aim at. */
final class HorseInfo {
	private HorseInfo() {}

	static boolean works() {
		//#if MC >= 1.16
		return true;
		//#else
		return false;
		//#endif
	}

	/** {speed in blocks per second, jump height in blocks, health, max health}, or null. */
	static float[] stats() {
		//#if MC >= 1.16
		Minecraft mc = Minecraft.getInstance();
		if (mc.player == null) {
			return null;
		}
		Entity e = mc.player.getVehicle();
		if (!(e instanceof AbstractHorse) && mc.crosshairPickEntity != null) {
			e = mc.crosshairPickEntity;
		}
		if (!(e instanceof AbstractHorse)) {
			return null;
		}
		AbstractHorse horse = (AbstractHorse) e;
		double speed = horse.getAttributeValue(Attributes.MOVEMENT_SPEED);
		double strength = horse.getAttributeValue(Attributes.JUMP_STRENGTH);
		// The game's own jump curve: how high a horse with this jump strength gets, in blocks.
		double jump = -0.1817584952 * strength * strength * strength + 3.689713992 * strength * strength + 2.128599134 * strength
				- 0.343930367;
		return new float[] {(float) (speed * 43.17), (float) jump, horse.getHealth(), horse.getMaxHealth()};
		//#else
		return null;
		//#endif
	}
}
