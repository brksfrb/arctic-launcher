package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.Replays;
import net.minecraft.client.Minecraft;
//#if MC >= 1.21
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.world.entity.Leashable;
//#else
import net.minecraft.client.renderer.entity.MobRenderer;
import net.minecraft.world.entity.Mob;
//#endif
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/** Hide only the leash when its holder cannot supply lighting or a culling box in replay. */
//#if MC >= 1.21
@Mixin(EntityRenderer.class)
//#else
@Mixin(MobRenderer.class)
//#endif
abstract class ReplayLeashRendererMixin {
	//#if MC >= 1.21.2
	@Redirect(method = {"shouldRender", "extract" + "RenderState"}, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/entity/Leashable;getLeashHolder()Lnet/minecraft/world/entity/Entity;"))
	//#elif MC >= 1.21
	@Redirect(method = {"shouldRender", "render"}, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/entity/Leashable;getLeashHolder()Lnet/minecraft/world/entity/Entity;"))
	//#elif MC >= 1.15
	@Redirect(method = {"shouldRender", "render"}, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/entity/Mob;getLeashHolder()Lnet/minecraft/world/entity/Entity;"))
	//#else
	@Redirect(method = {"shouldRender", "renderLeash"}, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/entity/Mob;getLeashHolder()Lnet/minecraft/world/entity/Entity;"))
	//#endif
	private Entity arctic$replayLeashHolder(
			//#if MC >= 1.21
			Leashable entity
			//#else
			Mob entity
			//#endif
	) {
		Entity holder = entity.getLeashHolder();
		if (holder != null && Replays.watching()
				&& Minecraft.getInstance().getEntityRenderDispatcher().getRenderer(holder) == null) {
			return null;
		}
		return holder;
	}
}
