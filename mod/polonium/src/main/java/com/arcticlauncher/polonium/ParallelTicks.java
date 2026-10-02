package com.arcticlauncher.polonium;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.RemotePlayer;
import net.minecraft.world.entity.Entity;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Other players (and mannequins) ticked on several threads. With thousands of players in
 * view, the client's own tick of each (smoothing its position, its walk
 * animation and head turn, its effects and pose) is a third of the time.
 * Each player's tick mostly touches only that player, so they're set aside
 * while the game ticks the level's entities, then ticked together on the
 * helper threads. What reaches outside a player is held back on its thread
 * ({@link #defer}) and done afterwards on the render thread, in order: moving
 * between the level's entity sections, particles, sounds. The world itself
 * is only read meanwhile (nothing else changes it during the entity tick).
 *
 * A failure in a parallel tick turns this off for the rest of the session
 * (logged), rather than risk it again. Off with -Dpolonium.parallelTicks=false.
 */
public final class ParallelTicks {
	private static final Logger LOG = LoggerFactory.getLogger("Polonium");
	private static volatile boolean enabled = !"false".equals(System.getProperty("polonium.parallelTicks"));
	/** Below this many players the threads aren't worth waking. */
	private static final int PARALLEL_MIN = 64;
	/** Jobs per thread, so a slow stretch of players doesn't keep the others waiting. */
	private static final int JOBS_PER_THREAD = 4;

	private static boolean collecting;
	private static volatile boolean running;
	private static final List<Entity> COLLECTED = new ArrayList<>();
	/** The deferred work of the job running on this thread (null: not in a parallel tick). */
	private static final ThreadLocal<List<Runnable>> DEFERRED = new ThreadLocal<>();

	private ParallelTicks() {}

	/** The level is about to tick its entities. */
	public static void begin() {
		collecting = enabled;
		COLLECTED.clear();
	}

	/** An entity is about to be ticked: true if it's set aside to be ticked with the others (the caller skips it). */
	public static boolean collect(Entity entity) {
		if (!collecting || running || !playerLike(entity) || !entity.getPassengers().isEmpty()) {
			return false;
		}
		COLLECTED.add(entity);
		return true;
	}

	/** Whether players are ticking in parallel right now. */
	public static boolean running() {
		return running;
	}

	/**
	 * Other players and mannequins (a server's bots are often mannequins):
	 * their client tick only smooths their movement and turning and steps
	 * their animations (no AI, no physics), like a player's.
	 */
	private static boolean playerLike(Entity entity) {
		Class<?> type = entity.getClass();
		return type == RemotePlayer.class || type == net.minecraft.client.entity.ClientMannequin.class;
	}

	/** Whether work reaching outside the entity being ticked must wait (see {@link #defer}). */
	public static boolean deferring() {
		return running && DEFERRED.get() != null;
	}

	/**
	 * Run a lookup in the level's entity lists ({@code lists}: the section or
	 * map searched) in turn with the other threads during parallel ticks, else
	 * as it is. Only threads searching the same lists wait for each other (see
	 * the *LookupMixin classes).
	 */
	public static <T> T inTurn(Object lists, java.util.function.Supplier<T> lookup) {
		if (!running) {
			return lookup.get();
		}
		synchronized (lists) {
			return lookup.get();
		}
	}

	/** Do this on the render thread once the parallel ticks are done. */
	public static void defer(Runnable work) {
		DEFERRED.get().add(work);
	}

	/** The level ticked its entities: now the ones set aside. */
	public static void end(ClientLevel level) {
		collecting = false;
		int count = COLLECTED.size();
		if (count == 0) {
			return;
		}
		if (count < PARALLEL_MIN || !enabled) {
			for (Entity entity : COLLECTED) {
				tickOne(level, entity);
			}
			COLLECTED.clear();
			return;
		}
		int parts = Math.min(count, (Workers.HELPERS + 1) * JOBS_PER_THREAD);
		List<List<Runnable>> deferred = new ArrayList<>(parts);
		List<Runnable> jobs = new ArrayList<>(parts);
		Throwable[] failure = new Throwable[1];
		for (int p = 0; p < parts; p++) {
			int from = count * p / parts;
			int to = count * (p + 1) / parts;
			List<Runnable> later = new ArrayList<>();
			deferred.add(later);
			jobs.add(() -> {
				DEFERRED.set(later);
				try {
					for (int i = from; i < to; i++) {
						try {
							tickOne(level, COLLECTED.get(i));
						} catch (Throwable e) {
							synchronized (failure) {
								if (failure[0] == null) {
									failure[0] = e;
								}
							}
						}
					}
				} finally {
					DEFERRED.remove();
				}
			});
		}
		running = true;
		try {
			Workers.runAll(jobs);
		} finally {
			running = false;
		}
		for (List<Runnable> later : deferred) {
			for (Runnable work : later) {
				work.run();
			}
		}
		COLLECTED.clear();
		if (failure[0] != null) {
			enabled = false;
			LOG.error("Polonium: ticking players in parallel failed; they're ticked one by one from now on", failure[0]);
		}
	}

	private static void tickOne(ClientLevel level, Entity entity) {
		if (!entity.isRemoved()) {
			level.tickNonPassenger(entity);
		}
	}
}
