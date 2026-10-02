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
	private final SkinAtlas atlas = new SkinAtlas();
	/** The texture itself: no offset, full scale. */
	private static final float[] OWN_TEXTURE = {0f, 0f, 1f, 1f};
	private final float[] cellUv = new float[4];
	/** Off with -Dpolonium.parallelPosing=false: every model is posed on the render thread. */
	private static final boolean PARALLEL_POSING = !"false".equals(System.getProperty("polonium.parallelPosing"));
	/** Per entity state, its models that the game also draws on top (enchantment glint): they stay on its path. */
	private final Map<Object, java.util.Set<Model<?>>> keepOnGamePath = new IdentityHashMap<>();
	private final GpuBatches batches = new GpuBatches("entity models", () -> GpuBatches.evictIdle(meshes, batches().frame()));

	public GpuEntities() {
		batches.beforeUpload(poses::computeAll);
		batches.afterFrame(() -> {
			poses.clear();
			atlas.endFrame();
		});
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

	/**
	 * The atlas cell for this render type's texture (Sampler0), or -1 to draw
	 * with the texture itself: not 64×64, or its pipeline moves texture
	 * coordinates around itself (a texture matrix).
	 */
	private int atlasCell(RenderType renderType) {
		if (!SkinAtlas.ENABLED) {
			return -1;
		}
		net.minecraft.client.renderer.rendertype.PreparedRenderType prepared = batches.prepare(renderType);
		if (prepared.pipeline().getShaderDefines().flags().contains("APPLY_TEXTURE_MATRIX")) {
			return -1;
		}
		for (net.minecraft.client.renderer.rendertype.PreparedRenderType.Texture texture : prepared.textures()) {
			if ("Sampler0".equals(texture.name())) {
				int cell = SkinAtlas.fits(texture.textureView()) ? atlas.cell(texture.textureView(), batches.frame()) : -1;
				debugAtlas(texture.textureView(), cell);
				return cell;
			}
		}
		debugAtlas(null, -2);
		return -1;
	}

	private static final boolean DEBUG_ATLAS = Boolean.getBoolean("polonium.debugAtlas");
	private final java.util.Map<String, Integer> atlasReasons = new java.util.HashMap<>();
	private long atlasReport;

	private void debugAtlas(com.mojang.blaze3d.textures.@org.jspecify.annotations.Nullable GpuTextureView view, int cell) {
		if (!DEBUG_ATLAS) {
			return;
		}
		String why = view == null ? "no Sampler0" : cell >= 0 ? "in atlas"
				: "not taken " + view.texture().getWidth(0) + "x" + view.texture().getHeight(0) + " mips=" + view.texture().getMipLevels()
						+ " base=" + view.baseMipLevel() + " " + view.texture().getFormat() + " " + view.texture().getLabel();
		atlasReasons.merge(why.length() > 120 ? why.substring(0, 120) : why, 1, Integer::sum);
		long now = System.nanoTime();
		if (now - atlasReport > 10_000_000_000L) {
			atlasReport = now;
			org.slf4j.LoggerFactory.getLogger("Polonium").info("Polonium atlas: {}", atlasReasons);
			atlasReasons.clear();
		}
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
		float[] uv = OWN_TEXTURE;
		com.mojang.blaze3d.textures.GpuTextureView atlasView = null;
		int cell = atlasCell(renderType);
		if (cell >= 0) {
			atlasView = atlas.view();
			float size = atlas.size();
			cellUv[0] = atlas.cellX(cell) / size;
			cellUv[1] = atlas.cellY(cell) / size;
			cellUv[2] = SkinAtlas.CELL / size;
			cellUv[3] = SkinAtlas.CELL / size;
			uv = cellUv;
		}
		InstanceData data = batches.add(renderType, mesh, atlasView);
		if (PARALLEL_POSING && ModelCopies.copyable(model)) {
			// Posed later on the helper threads, each with its own copy of the model.
			poses.defer(mesh, model, submit.state(), submit.pose(), submit.tintedColor(), submit.overlayCoords(), submit.lightCoords(), uv,
					data);
			return;
		}
		// What the game does before turning the model into vertices.
		model.setupAnim(submit.state());
		poses.snapshot(mesh, submit.pose(), submit.tintedColor(), submit.overlayCoords(), submit.lightCoords(), uv, data);
	}
}
