//#if MC < 1.12
package com.arcticlauncher.legacy.mixin;

import net.minecraft.advancement.Achievement;
import net.minecraft.client.gui.AchievementNotification;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** Whether Minecraft's achievement pop-up is showing, so Arctic's go below it. */
@Mixin(AchievementNotification.class)
public interface AchievementNotificationAccess {
	@Accessor("achievement")
	Achievement arctic$achievement();

	@Accessor("time")
	long arctic$time();

	@Accessor("permanent")
	boolean arctic$permanent();
}
//#endif
