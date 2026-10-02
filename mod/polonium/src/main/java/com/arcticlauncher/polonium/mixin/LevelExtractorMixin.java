//#if MC >= 26.2
package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.ParallelExtract;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Camera;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.client.renderer.state.level.LevelRenderState;
import net.minecraft.util.Mth;
import net.minecraft.world.TickRateManager;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.phys.Vec3;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The game's {@code extractVisibleEntities}, with the per-entity work spread
 * over threads: which entities are visible, then their render states, are
 * worked out on several threads (only for renderers known to be safe, see
 * {@link ParallelExtract}); everything with side effects stays on this
 * thread, and states are added in the game's order. With few entities, or when Polonium has stepped aside,
 * the game's own code runs.
 *
 * Applied after other mods' hooks (a priority above the default 1000): their
 * callbacks at the head of extractVisibleEntities run before this one takes
 * the method over (entity culling records the frame's frustum there).
 */
@Mixin(value = LevelExtractor.class, priority = 1500)
abstract class LevelExtractorMixin {
	@Shadow
	@Final
	private Minecraft minecraft;

	@Shadow
	@Final
	private LevelRenderer levelRenderer;

	@Shadow
	private ClientLevel level;

	@Shadow
	public abstract boolean isEntityVisible(Entity entity, Frustum frustum, double camX, double camY, double camZ);

	@Shadow
	protected abstract EntityRenderState extractEntity(Entity entity, float partialTicks);

	@Inject(method = "extractVisibleEntities", at = @At("HEAD"), cancellable = true)
	private void polonium$parallel(Camera camera, Frustum frustum, DeltaTracker deltaTracker, LevelRenderState output, CallbackInfo ci) {
		if (!ParallelExtract.enabled()) {
			return;
		}
		com.arcticlauncher.polonium.Timeline.start(com.arcticlauncher.polonium.Timeline.Step.EXTRACT_ENTITIES);
		Vec3 cameraPos = camera.position();
		double camX = cameraPos.x();
		double camY = cameraPos.y();
		double camZ = cameraPos.z();
		TickRateManager tickRateManager = this.minecraft.level.tickRateManager();
		Entity.setViewScale(
				Mth.clamp(this.minecraft.options.getEffectiveRenderDistance() / 8.0, 1.0, 2.5) * this.minecraft.options.entityDistanceScaling().get());
		List<Entity> all = new ArrayList<>();
		for (Entity entity : this.level.entitiesForRendering()) {
			all.add(entity);
		}
		com.arcticlauncher.polonium.Workers.load(all.size());
		EntityRenderDispatcher dispatcher = this.levelRenderer.entityRenderDispatcher();
		boolean[] trustedAll = new boolean[all.size()];
		boolean[] seen = ParallelExtract.visible(all, dispatcher, e -> this.isEntityVisible(e, frustum, camX, camY, camZ), trustedAll);
		List<Entity> visible = new ArrayList<>(all.size());
		float[] partials = new float[all.size()];
		boolean[] trusted = new boolean[all.size()];
		// The same for every entity but those the tick rate freezes.
		float running = deltaTracker.getGameTimeDeltaPartialTick(true);
		float frozen = deltaTracker.getGameTimeDeltaPartialTick(false);
		for (int i = 0; i < all.size(); i++) {
			Entity entity = all.get(i);
			if (seen[i]
					&& (entity != camera.entity() || camera.isDetached()
							|| camera.entity() instanceof LivingEntity && ((LivingEntity) camera.entity()).isSleeping())
					&& (!(entity instanceof LocalPlayer) || camera.entity() == entity)) {
				if (entity.tickCount == 0) {
					entity.xOld = entity.getX();
					entity.yOld = entity.getY();
					entity.zOld = entity.getZ();
				}
				trusted[visible.size()] = trustedAll[i];
				partials[visible.size()] = tickRateManager.isEntityFrozen(entity) ? frozen : running;
				visible.add(entity);
			}
		}
		ParallelExtract.extract(visible, partials, trusted, this::extractEntity, output.entityRenderStates);
		com.arcticlauncher.polonium.Timeline.end(com.arcticlauncher.polonium.Timeline.Step.EXTRACT_ENTITIES);
		ci.cancel();
	}
}
//#endif
