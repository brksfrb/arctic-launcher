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
	/** Shapes drawn with another model's poses (see {@link ModelMesh#borrowing}), by (model, owner). */
	private final Map<java.util.List<Model<?>>, ModelMesh> borrowedMeshes = new java.util.HashMap<>();
	private final PartPoses poses = new PartPoses();
	private final SkinAtlas atlas = new SkinAtlas();
	/** The texture itself: no offset, full scale. */
	private static final float[] OWN_TEXTURE = {0f, 0f, 1f, 1f};
	private final float[] cellUv = new float[4];
	/** Off with -Dpolonium.parallelPosing=false: every model is posed on the render thread. */
	private static final boolean PARALLEL_POSING = !"false".equals(System.getProperty("polonium.parallelPosing"));
	/** Per entity state, its models that the game also draws on top (enchantment glint): they stay on its path. */
	private final Map<Object, java.util.Set<Model<?>>> keepOnGamePath = new IdentityHashMap<>();
	private final GpuBatches batches = new GpuBatches("entity models", () -> {
		GpuBatches.evictIdle(meshes, batches().frame());
		GpuBatches.evictIdle(borrowedMeshes, batches().frame());
	});

	public GpuEntities() {
		batches.beforeUpload(() -> {
			Crowd.computeAll();
			Crowd.linkBorrowed();
			poses.computeAll();
		});
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

	/** Whether models of this render type can be drawn on the GPU path at all. */
	static boolean drawable(RenderType renderType) {
		return InstancedPipelines.supports(renderType.pipeline(), ENTITY_SHADER);
	}

	private static boolean takeable(ModelFeatureRenderer.Submit<?> submit) {
		return submit.sprite() == null && submit.sheetedDecalPose() == null
				&& InstancedPipelines.supports(submit.renderType().pipeline(), ENTITY_SHADER);
	}

	/** Take this submit onto the GPU path; false to let the game build its vertices. */
	public boolean capture(ModelFeatureRenderer.Submit<?> submit) {
		if (submit.state() instanceof Crowd.Bucket bucket) {
			// Never the game's way: its "state" is the bucket.
			addCrowd(bucket);
			return true;
		}
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

	/** A crowd bucket's players (see {@link Crowd}): one instance each, in this group. */
	private void addCrowd(Crowd.Bucket bucket) {
		if (GpuBatches.modelMod() || !batches.preparing()) {
			return;
		}
		try {
			ModelMesh mesh = bucket.owner == null ? mesh(bucket.model) : borrowedMesh(bucket);
			RenderType renderType = bucket.renderType;
			if (!bucket.resolved()) {
				resolve(bucket);
			}
			com.mojang.blaze3d.textures.GpuTextureView atlasView = null;
			int cell = bucket.atlasTexture != null ? atlas.cell(bucket.atlasTexture, batches.frame()) : -1;
			if (cell >= 0) {
				renderType = bucket.batchOwner.renderType;
				atlasView = atlas.view();
				float size = atlas.size();
				bucket.uv[0] = atlas.cellX(cell) / size;
				bucket.uv[1] = atlas.cellY(cell) / size;
				bucket.uv[2] = SkinAtlas.CELL / size;
				bucket.uv[3] = SkinAtlas.CELL / size;
			} else {
				System.arraycopy(OWN_TEXTURE, 0, bucket.uv, 0, 4);
			}
			if (renderType.hasBlending()) {
				Crowd.sortFarToNear(bucket.members);
			}
			for (int i = 0; i < bucket.members.size(); i++) {
				InstanceData data = batches.add(renderType, mesh, atlasView);
				Crowd.target(bucket.members.getInt(i), data, data.reserve(mesh.texelsPerInstance), mesh, bucket.uv);
			}
		} catch (RuntimeException | LinkageError e) {
			GpuBatches.disable("couldn't take a crowd onto the GPU", e);
		}
	}

	/**
	 * Crowd buckets whose render types differ only in their (atlas) texture,
	 * by everything else about them: they share batches, made with the first
	 * one's render type. That way each skin's render type needn't be prepared
	 * every frame (with thousands of skins, that added up).
	 */
	private final Map<java.util.List<Object>, Crowd.Bucket> families = new java.util.HashMap<>();
	private static final int MAX_FAMILIES = 256;

	private void resolve(Crowd.Bucket bucket) {
		RenderType renderType = bucket.renderType;
		bucket.atlasTexture = null;
		bucket.batchOwner = bucket;
		net.minecraft.client.renderer.rendertype.PreparedRenderType prepared = batches.prepare(renderType);
		if (!SkinAtlas.ENABLED || prepared.pipeline().getShaderDefines().flags().contains("APPLY_TEXTURE_MATRIX")) {
			return;
		}
		java.util.List<Object> family = new java.util.ArrayList<>();
		family.add(bucket.model);
		family.add(prepared.pipeline());
		family.add(prepared.outputTarget());
		family.add(prepared.scissorState());
		com.mojang.blaze3d.textures.GpuTextureView texture = null;
		for (net.minecraft.client.renderer.rendertype.PreparedRenderType.Texture t : prepared.textures()) {
			family.add(t.name());
			if ("Sampler0".equals(t.name())) {
				texture = t.textureView();
				com.mojang.blaze3d.textures.GpuSampler sampler = t.sampler();
				family.add(java.util.List.of(sampler.getAddressModeU(), sampler.getAddressModeV(), sampler.getMinFilter(), sampler.getMagFilter(),
						sampler.getMaxAnisotropy(), sampler.getMaxLod()));
			} else {
				family.add(t.textureView());
				family.add(t.sampler());
			}
		}
		if (texture == null || !SkinAtlas.fits(texture)) {
			return;
		}
		if (families.size() > MAX_FAMILIES) {
			families.clear();
		}
		bucket.atlasTexture = texture;
		// The owner's texture must still be there: its render type is prepared for the batches.
		Crowd.Bucket owner = families.get(family);
		if (owner == null || owner.atlasTexture == null || owner.atlasTexture.texture().isClosed()) {
			owner = bucket;
			families.put(family, bucket);
		}
		bucket.batchOwner = owner;
	}

	private ModelMesh borrowedMesh(Crowd.Bucket bucket) {
		java.util.List<Model<?>> key = java.util.List.of(bucket.model, bucket.owner);
		ModelMesh mesh = borrowedMeshes.get(key);
		if (mesh == null) {
			mesh = ModelMesh.borrowing(bucket.model, bucket.borrowIndex);
			borrowedMeshes.put(key, mesh);
		}
		return mesh;
	}

	private ModelMesh mesh(Model<?> model) {
		ModelMesh mesh = meshes.get(model);
		if (mesh == null) {
			mesh = ModelMesh.build(model);
			meshes.put(model, mesh);
		}
		return mesh;
	}

	@SuppressWarnings({"unchecked", "rawtypes"})
	private void add(ModelFeatureRenderer.Submit<?> submit, RenderType renderType) {
		Model model = submit.model();
		ModelMesh mesh = mesh(model);
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
