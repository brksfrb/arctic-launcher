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
 * over threads: which entities are visible is decided here, in order, as the
 * game does; their render states are then made on several threads (only for
 * renderers known to be safe, see {@link ParallelExtract}); and they're added
 * in the game's order. With few entities, or when Polonium has stepped aside,
 * the game's own code runs.
 */
@Mixin(LevelExtractor.class)
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
		Vec3 cameraPos = camera.position();
		double camX = cameraPos.x();
		double camY = cameraPos.y();
		double camZ = cameraPos.z();
		TickRateManager tickRateManager = this.minecraft.level.tickRateManager();
		Entity.setViewScale(
				Mth.clamp(this.minecraft.options.getEffectiveRenderDistance() / 8.0, 1.0, 2.5) * this.minecraft.options.entityDistanceScaling().get());
		List<Entity> visible = new ArrayList<>();
		List<Float> partials = new ArrayList<>();
		for (Entity entity : this.level.entitiesForRendering()) {
			if (this.isEntityVisible(entity, frustum, camX, camY, camZ)
					&& (entity != camera.entity() || camera.isDetached()
							|| camera.entity() instanceof LivingEntity && ((LivingEntity) camera.entity()).isSleeping())
					&& (!(entity instanceof LocalPlayer) || camera.entity() == entity)) {
				if (entity.tickCount == 0) {
					entity.xOld = entity.getX();
					entity.yOld = entity.getY();
					entity.zOld = entity.getZ();
				}
				visible.add(entity);
				partials.add(deltaTracker.getGameTimeDeltaPartialTick(!tickRateManager.isEntityFrozen(entity)));
			}
		}
		EntityRenderDispatcher dispatcher = this.levelRenderer.entityRenderDispatcher();
		EntityRenderState[] states = ParallelExtract.extract(visible, partials, dispatcher, this::extractEntity);
		for (EntityRenderState state : states) {
			output.entityRenderStates.add(state);
		}
		ci.cancel();
	}
}
