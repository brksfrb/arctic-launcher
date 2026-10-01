package com.arcticlauncher.polonium;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.world.entity.Entity;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Entity render states made on several threads. Making one mostly reads the
 * entity (position, pose, equipment) into a fresh state object, so entities
 * can be done at once, as long as their renderer keeps nothing of its own
 * while doing it. Only Minecraft's own living-entity renderers are trusted;
 * any other entity is done on the render thread, as the game does. Mods that
 * change entity textures or models per entity (ETF, EMF) keep their own
 * state while doing this, so with them installed Polonium steps aside.
 *
 * If a parallel frame ever fails, it's redone the game's way and parallel
 * extraction stays off. Turn it off with -Dpolonium.parallelExtract=false.
 */
public final class ParallelExtract {
	private static final Logger LOG = LoggerFactory.getLogger("Polonium");
	/** Below this many visible entities, threads aren't worth waking. */
	private static final int PARALLEL_MIN = 48;
	private static final Map<Class<?>, Boolean> TRUSTED = new ConcurrentHashMap<>();
	private static volatile boolean enabled = !"false".equals(System.getProperty("polonium.parallelExtract"))
			&& !loaded("entity_texture_features", "entity_model_features", "figura");
	private static boolean announced;

	/** The game's own per-entity extraction (with whatever other mods add to it). */
	public interface Extractor {
		EntityRenderState extract(Entity entity, float partialTicks);
	}

	private ParallelExtract() {}

	public static boolean enabled() {
		return enabled;
	}

	/** Every visible entity's render state, in order. */
	public static EntityRenderState[] extract(List<Entity> entities, List<Float> partials, EntityRenderDispatcher dispatcher,
			Extractor extractor) {
		int count = entities.size();
		EntityRenderState[] states = new EntityRenderState[count];
		if (count < PARALLEL_MIN) {
			serial(entities, partials, extractor, states, null);
			return states;
		}
		if (!announced) {
			announced = true;
			LOG.info("Polonium: entity render states made on several threads");
		}
		// Untrusted renderers first, on this thread, in order.
		boolean[] parallel = new boolean[count];
		List<Integer> trusted = new ArrayList<>(count);
		for (int i = 0; i < count; i++) {
			if (trusted(dispatcher.getRenderer(entities.get(i)))) {
				parallel[i] = true;
				trusted.add(i);
			} else {
				states[i] = extractor.extract(entities.get(i), partials.get(i));
			}
		}
		int parts = Workers.HELPERS + 1;
		List<Runnable> jobs = new ArrayList<>(parts);
		for (int p = 0; p < parts; p++) {
			int from = trusted.size() * p / parts;
			int to = trusted.size() * (p + 1) / parts;
			jobs.add(() -> {
				for (int k = from; k < to; k++) {
					int i = trusted.get(k);
					states[i] = extractor.extract(entities.get(i), partials.get(i));
				}
			});
		}
		try {
			Workers.runAll(jobs);
		} catch (RuntimeException | Error e) {
			enabled = false;
			LOG.error("Polonium: making entity render states on several threads failed; back to one thread from now on", e);
			serial(entities, partials, extractor, states, parallel);
		}
		return states;
	}

	/** On this thread: every entity, or only those marked in {@code only}. */
	private static void serial(List<Entity> entities, List<Float> partials, Extractor extractor, EntityRenderState[] states,
			boolean[] only) {
		for (int i = 0; i < entities.size(); i++) {
			if (only == null || only[i]) {
				states[i] = extractor.extract(entities.get(i), partials.get(i));
			}
		}
	}

	/** Minecraft's own living-entity renderers only. */
	private static boolean trusted(EntityRenderer<?, ?> renderer) {
		return TRUSTED.computeIfAbsent(renderer.getClass(),
				type -> LivingEntityRenderer.class.isAssignableFrom(type) && type.getName().startsWith("net.minecraft."));
	}

	private static boolean loaded(String... ids) {
		for (String id : ids) {
			if (FabricLoader.getInstance().isModLoaded(id)) {
				return true;
			}
		}
		return false;
	}
}
