package com.arcticlauncher.polonium.gpu;

import java.util.IdentityHashMap;
import java.util.Map;
import net.minecraft.client.model.Model;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.resources.Identifier;

/**
 * Entity models (bodies, armor, capes) on the GPU: each model's shape is
 * uploaded once ({@link ModelMesh}); per entity only the part poses go up
 * ({@link PartPoses}). One of these per {@link ModelFeatureRenderer}; render
 * thread only.
 */
public final class GpuEntities {
	private static final Identifier ENTITY_SHADER = Identifier.withDefaultNamespace("core/entity");
	private final Map<Model<?>, ModelMesh> meshes = new IdentityHashMap<>();
	private final PartPoses poses = new PartPoses();
	/** Per entity state, its models that the game also draws on top (enchantment glint): they stay on its path. */
	private final Map<Object, java.util.Set<Model<?>>> keepOnGamePath = new IdentityHashMap<>();
	private final GpuBatches batches = new GpuBatches("entity models", () -> GpuBatches.evictIdle(meshes, batches().frame()));

	public GpuEntities() {
		batches.beforeUpload(poses::computeAll);
		batches.afterFrame(poses::clear);
	}

	public GpuBatches batches() {
		return batches;
	}

	/**
	 * Before the frame is prepared: an entity's model that the game also draws
	 * with a render type Polonium doesn't take (enchantment glint on armor, say)
	 * stays entirely on the game's path. That overlay is drawn at exactly the
	 * model's depth, which only the game's own (CPU) positions match.
	 */
	public void scan(java.util.List<?> submits) {
		keepOnGamePath.clear();
		for (Object node : submits) {
			if (node instanceof ModelFeatureRenderer.Submit<?> submit && !takeable(submit)) {
				keepOnGamePath.computeIfAbsent(submit.state(), state -> java.util.Collections.newSetFromMap(new IdentityHashMap<>()))
						.add(submit.model());
			}
		}
	}

	private boolean keptOnGamePath(ModelFeatureRenderer.Submit<?> submit) {
		java.util.Set<Model<?>> models = keepOnGamePath.isEmpty() ? null : keepOnGamePath.get(submit.state());
		return models != null && models.contains(submit.model());
	}

	private static boolean takeable(ModelFeatureRenderer.Submit<?> submit) {
		return submit.sprite() == null && submit.sheetedDecalPose() == null
				&& InstancedPipelines.supports(submit.renderType().pipeline(), ENTITY_SHADER);
	}

	/** Take this submit onto the GPU path; false to let the game build its vertices. */
	public boolean capture(ModelFeatureRenderer.Submit<?> submit) {
		if (GpuBatches.modelMod() || !batches.preparing() || !takeable(submit) || keptOnGamePath(submit)) {
			return false;
		}
		RenderType renderType = submit.renderType();
		try {
			add(submit, renderType);
			return true;
		} catch (RuntimeException | LinkageError e) {
			GpuBatches.disable("couldn't take a model onto the GPU", e);
			return false;
		}
	}

	@SuppressWarnings({"unchecked", "rawtypes"})
	private void add(ModelFeatureRenderer.Submit<?> submit, RenderType renderType) {
		Model model = submit.model();
		ModelMesh mesh = meshes.get(model);
		if (mesh == null) {
			mesh = ModelMesh.build(model);
			meshes.put(model, mesh);
		}
		InstanceData data = batches.add(renderType, mesh);
		// What the game does before turning the model into vertices.
		model.setupAnim(submit.state());
		poses.snapshot(mesh, submit.pose(), submit.tintedColor(), submit.overlayCoords(), submit.lightCoords(), data);
	}
}
