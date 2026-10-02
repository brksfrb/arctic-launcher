package com.arcticlauncher.polonium.gpu;

import com.arcticlauncher.polonium.GlyphRuns;
import com.arcticlauncher.polonium.NameTagCache;
import java.util.Arrays;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.renderer.SubmitNodeCollection;
import net.minecraft.client.renderer.SubmitNodeStorage;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.client.renderer.feature.NameTagFeatureRenderer;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.network.chat.Component;
import net.minecraft.util.ARGB;
import net.minecraft.util.LightCoordsUtil;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix4f;
import org.jspecify.annotations.Nullable;

/**
 * The crowd's name tags (see {@link Crowd}): worked out as the game does
 * ({@code EntityRenderer.submitNameDisplay}, {@code SubmitNodeCollection.submitNameTag})
 * but kept in two lists, one per way tags are drawn, each handed to the game
 * as a single tag. When the name tag renderer gets to that tag it draws the
 * whole list on the GPU ({@link GpuText}), see-through ones back to front as
 * the game sorts them. Thousands of tags then skip the game's per-tag
 * submits, sorting and grouping.
 *
 * Each player keeps its tags' width, layout and glyph runs ({@link Look},
 * on its recipe) while the text stays the same, so a frame's tag is its pose
 * and light plus runs already at hand. Render thread only.
 */
public final class CrowdTags {
	/** Stand-ins for the lists, as the text of the tag handed to the game. */
	private static final Component NORMAL = Component.empty();
	private static final Component SEE_THROUGH = Component.empty();
	/** As the game: a see-through tag's shadow color. */
	private static final int TAG_COLOR = -2130706433;
	/** The ways a tag is drawn: the solid pass of a see-through tag, its see-through pass, and a discreet (sneaking) tag. */
	private static final int SOLID_PASS = 0;
	private static final int SEE_THROUGH_PASS = 1;
	private static final int DISCREET = 2;
	/** This frame's tag turn (to face the camera) and scale, the same for every tag. */
	private static final Matrix4f FACING = new Matrix4f();

	private static final Tags normal = new Tags();
	private static final Tags seeThrough = new Tags();
	private static int background;

	private CrowdTags() {}

	/**
	 * One tag text of one player, laid out for each way it's drawn, while the
	 * text (and fonts, and the background opacity) stay the same.
	 */
	static final class Look {
		final Component text;
		final float width;
		private final int generation;
		private final GlyphRuns.Run[][] runs = new GlyphRuns.Run[3][];
		private final float[] y = new float[3];
		private final int[] background = new int[3];

		Look(Component text, Font font) {
			this.text = text;
			this.width = font.width(text);
			this.generation = NameTagCache.generation();
		}

		boolean current(Component text) {
			return this.text == text && generation == NameTagCache.generation();
		}

		/** The runs for one way of drawing it if laid out already (read-only: safe on any thread), else null. */
		GlyphRuns.Run @Nullable [] kept(int way, float y, int background) {
			GlyphRuns.Run[] kept = runs[way];
			return kept != null && this.y[way] == y && this.background[way] == background ? kept : null;
		}

		/** The runs for one way of drawing it, laid out (and recorded) the first time. */
		GlyphRuns.Run[] runs(int way, float y, int background, Font font) {
			GlyphRuns.Run[] kept = runs[way];
			if (kept != null && this.y[way] == y && this.background[way] == background) {
				return kept;
			}
			if (DEBUG) {
				MADE.merge("runs " + (kept == null ? "first" : this.y[way] != y ? "y" : "background") + " way " + way, 1, Integer::sum);
			}
			int color = way == SOLID_PASS ? -1 : TAG_COLOR;
			int shownBackground = way == SOLID_PASS ? 0 : background;
			Font.DisplayMode mode = way == SEE_THROUGH_PASS ? Font.DisplayMode.SEE_THROUGH : Font.DisplayMode.NORMAL;
			Font.PreparedText layout = font.prepareText(text.getVisualOrderText(), -width / 2.0F, y, color, false, false, shownBackground);
			kept = GlyphRuns.runs(layout, mode);
			runs[way] = kept;
			this.y[way] = y;
			this.background[way] = background;
			return kept;
		}
	}

	/** One list of tags, as arrays (thousands a frame), filled in on the helper threads (see {@link #fill}). */
	private static final class Tags {
		int count;
		Look[] look = new Look[256];
		byte[] way = new byte[256];
		float[] y = new float[256];
		int[] light = new int[256];
		float[] distance = new float[256];
		float[] pose = new float[256 * 16];
		int farthest = -1;

		/** Room for {@code count} tags, set with {@link #set}. */
		void size(int count) {
			if (count > look.length) {
				int size = Math.max(look.length * 2, count);
				look = Arrays.copyOf(look, size);
				way = Arrays.copyOf(way, size);
				y = Arrays.copyOf(y, size);
				light = Arrays.copyOf(light, size);
				distance = Arrays.copyOf(distance, size);
				pose = Arrays.copyOf(pose, size * 16);
			}
			this.count = count;
		}

		void set(int i, Look look, int way, float y, int light, Matrix4f pose) {
			this.look[i] = look;
			this.way[i] = (byte) way;
			this.y[i] = y;
			this.light[i] = light;
			pose.get(this.pose, i * 16);
			distance[i] = pose.m30() * pose.m30() + pose.m31() * pose.m31() + pose.m32() * pose.m32();
		}

		void findFarthest() {
			farthest = -1;
			for (int i = 0; i < count; i++) {
				if (farthest < 0 || distance[i] > distance[farthest]) {
					farthest = i;
				}
			}
		}

		void clear() {
			Arrays.fill(look, 0, count, null);
			count = 0;
			farthest = -1;
		}
	}

	/** Whether the crowd's tags can go this way: only when the GPU draws name tags. */
	static boolean usable() {
		return GpuText.enabled() && !GpuBatches.disabled();
	}

	static void beginFrame() {
		normal.clear();
		seeThrough.clear();
		facingReady = false;
		background = ARGB.color(Minecraft.getInstance().gameRenderer.gameRenderState().optionsRenderState.getBackgroundOpacity(0.25F), -16777216);
	}

	// Per queued player (see CrowdFrame): which tags it has, and where they go in the lists.
	private static final int SCORE = 1;
	private static final int NAME = 2;
	private static final int SEE_THROUGH_TOO = 4;
	private static int[] planned = new int[256];
	private static CrowdRecipe[] plannedRecipe = new CrowdRecipe[256];
	private static int[] normalAt = new int[256];
	private static int[] seeThroughAt = new int[256];

	/** A queued player without crowd tags (the game draws its tags, or it has none). */
	static void none(int entity) {
		ensurePlanned(entity);
		planned[entity] = 0;
		plannedRecipe[entity] = null;
	}

	/**
	 * A queued player's score line and name, as {@code AvatarRenderer.submitNameDisplay}
	 * has them: their texts laid out if new (here, on the render thread:
	 * fonts aren't safe on several threads); placed later, in {@link #fill}.
	 */
	static void plan(int entity, AvatarRenderState state, CrowdRecipe recipe, CameraRenderState camera) {
		ensurePlanned(entity);
		int tags = 0;
		if (state.nameTagAttachment != null) {
			if (state.scoreText != null) {
				recipe.scoreLook = look(recipe.scoreLook, state.scoreText);
				tags |= SCORE;
			}
			if (state.nameTag != null) {
				recipe.nameLook = look(recipe.nameLook, state.nameTag);
				tags |= NAME;
			}
			if (!state.isDiscrete) {
				tags |= SEE_THROUGH_TOO;
			}
		}
		planned[entity] = tags;
		plannedRecipe[entity] = recipe;
		if (tags != 0 && !facingReady) {
			// As the game: turn to face the camera, then scale (worked out once, applied to every tag).
			FACING.rotation(camera.orientation).scale(0.025F, -0.025F, 0.025F);
			facingReady = true;
		}
	}

	/** Room for players {@code [0, count)} (before {@link #planKept} on several threads). */
	static void ensure(int count) {
		if (count > 0) {
			ensurePlanned(count - 1);
		}
	}

	/** This frame's tag turn, from the camera (before {@link #planKept} on several threads). */
	static void facing(CameraRenderState camera) {
		if (!facingReady) {
			FACING.rotation(camera.orientation).scale(0.025F, -0.025F, 0.025F);
			facingReady = true;
		}
	}

	/** Whether the player's tag texts are laid out already (then {@link #planKept} may plan it on any thread). */
	static boolean laidOut(AvatarRenderState state, CrowdRecipe recipe) {
		if (state.nameTagAttachment == null) {
			return true;
		}
		return (state.scoreText == null || recipe.scoreLook != null && recipe.scoreLook.current(state.scoreText))
				&& (state.nameTag == null || recipe.nameLook != null && recipe.nameLook.current(state.nameTag));
	}

	/** {@link #plan} for a player whose texts are laid out ({@link #laidOut}), after {@link #ensure} and {@link #facing}: safe on any thread. */
	static void planKept(int entity, AvatarRenderState state, CrowdRecipe recipe) {
		int tags = 0;
		if (state.nameTagAttachment != null) {
			if (state.scoreText != null) {
				tags |= SCORE;
			}
			if (state.nameTag != null) {
				tags |= NAME;
			}
			if (!state.isDiscrete) {
				tags |= SEE_THROUGH_TOO;
			}
		}
		planned[entity] = tags;
		plannedRecipe[entity] = recipe;
	}

	private static void ensurePlanned(int entity) {
		if (entity >= planned.length) {
			int size = Math.max(planned.length * 2, entity + 1);
			planned = Arrays.copyOf(planned, size);
			plannedRecipe = Arrays.copyOf(plannedRecipe, size);
			normalAt = Arrays.copyOf(normalAt, size);
			seeThroughAt = Arrays.copyOf(seeThroughAt, size);
		}
	}

	/** Every queued player planned: each one's place in the lists. */
	static void layOut(int count) {
		int normalCount = 0;
		int seeThroughCount = 0;
		for (int e = 0; e < count; e++) {
			int tags = planned[e];
			int n = ((tags & SCORE) != 0 ? 1 : 0) + ((tags & NAME) != 0 ? 1 : 0);
			normalAt[e] = normalCount;
			normalCount += n;
			seeThroughAt[e] = seeThroughCount;
			seeThroughCount += (tags & SEE_THROUGH_TOO) != 0 ? n : 0;
		}
		normal.size(normalCount);
		seeThrough.size(seeThroughCount);
	}

	/** A player's tags into their places (on a helper thread); {@code pose} is where it was submitted. */
	static void fill(int entity, AvatarRenderState state, Matrix4f pose, Matrix4f scratch) {
		int tags = planned[entity];
		if (tags == 0) {
			return;
		}
		CrowdRecipe recipe = plannedRecipe[entity];
		int offset = state.showExtraEars ? -10 : 0;
		boolean seeThroughToo = (tags & SEE_THROUGH_TOO) != 0;
		int n = normalAt[entity];
		int s = seeThroughAt[entity];
		Vec3 at = state.nameTagAttachment;
		float lift = 0;
		if ((tags & SCORE) != 0) {
			place(scratch.set(pose).translate((float) at.x, (float) (at.y + 0.5), (float) at.z).mul(FACING), recipe.scoreLook, offset,
					seeThroughToo, state.lightCoords, n++, s++);
			// The name goes above the score line.
			lift = 9.0F * 1.15F * 0.025F;
		}
		if ((tags & NAME) != 0) {
			place(scratch.set(pose).translate(0.0F, lift, 0.0F).translate((float) at.x, (float) (at.y + 0.5), (float) at.z).mul(FACING),
					recipe.nameLook, offset, seeThroughToo, state.lightCoords, n, s);
		}
	}

	private static void place(Matrix4f pose, Look look, int offset, boolean seeThroughToo, int light, int n, int s) {
		if (seeThroughToo) {
			normal.set(n, look, SOLID_PASS, offset, LightCoordsUtil.lightCoordsWithEmission(light, 2), pose);
			seeThrough.set(s, look, SEE_THROUGH_PASS, offset, light, pose);
		} else {
			normal.set(n, look, DISCREET, offset, light, pose);
		}
	}

	/** Every tag in place: where the farthest of each list is. */
	static void finish() {
		normal.findFarthest();
		seeThrough.findFarthest();
	}

	private static Look look(@Nullable Look kept, Component text) {
		if (kept != null && kept.current(text)) {
			return kept;
		}
		if (DEBUG) {
			String why = kept == null ? "new" : kept.text != text ? (kept.text.equals(text) ? "same text, new object" : "changed") : "fonts";
			MADE.merge(why + " " + text.getClass().getSimpleName(), 1, Integer::sum);
			long now = System.nanoTime();
			if (now - lastDebug > 10_000_000_000L) {
				lastDebug = now;
				org.slf4j.LoggerFactory.getLogger("Polonium").info("Polonium crowd tags laid out: {}", MADE);
				MADE.clear();
			}
		}
		return new Look(text, Minecraft.getInstance().font);
	}

	/** -Dpolonium.debugTags=true: every 10 s, log why the crowd's tags were laid out again. */
	private static final boolean DEBUG = Boolean.getBoolean("polonium.debugTags");
	private static final java.util.Map<String, Integer> MADE = new java.util.HashMap<>();
	private static long lastDebug;

	private static boolean facingReady;

	/** All entities are in: each list goes to the game as one tag, where the farthest of its tags is. */
	static void endSubmits(SubmitNodeStorage storage) {
		SubmitNodeCollection collection = storage.order(0);
		if (normal.count > 0 && normal.farthest >= 0) {
			collection.nameTags.submit(standIn(normal, NORMAL, Font.DisplayMode.NORMAL));
		}
		if (seeThrough.count > 0 && seeThrough.farthest >= 0) {
			collection.seeThroughNameTags.submit(standIn(seeThrough, SEE_THROUGH, Font.DisplayMode.SEE_THROUGH));
		}
	}

	private static NameTagFeatureRenderer.Submit standIn(Tags tags, Component marker, Font.DisplayMode mode) {
		Matrix4f pose = new Matrix4f().set(tags.pose, tags.farthest * 16);
		return new NameTagFeatureRenderer.Submit(pose, 0, 0, marker, 0, -1, 0, mode);
	}

	/** Whether this tag text stands for a list of the crowd's tags. */
	public static boolean isList(Component text) {
		return text == NORMAL || text == SEE_THROUGH;
	}

	/**
	 * The name tag renderer got to a list's stand-in: every tag in it, drawn
	 * on the GPU from its player's kept runs; see-through tags back to front.
	 */
	public static void draw(Component marker, GpuText gpu, Font font) {
		Tags tags = marker == NORMAL ? normal : seeThrough;
		Font.DisplayMode mode = marker == NORMAL ? Font.DisplayMode.NORMAL : Font.DisplayMode.SEE_THROUGH;
		int[] order = new int[tags.count];
		for (int i = 0; i < order.length; i++) {
			order[i] = i;
		}
		if (mode == Font.DisplayMode.SEE_THROUGH) {
			CrowdFrame.sortFarToNear(order, order.length, tags.distance);
		}
		int count = order.length;
		GlyphRuns.Run[][] runs = new GlyphRuns.Run[count][];
		// Kept runs, on the helper threads (most tags, every frame after their first).
		int parts = count < 256 ? 1 : com.arcticlauncher.polonium.Workers.PARTS;
		if (parts == 1) {
			keptRuns(tags, order, runs, 0, count);
		} else {
			java.util.List<Runnable> jobs = new java.util.ArrayList<>(parts);
			for (int p = 0; p < parts; p++) {
				int from = count * p / parts;
				int to = count * (p + 1) / parts;
				jobs.add(() -> keptRuns(tags, order, runs, from, to));
			}
			com.arcticlauncher.polonium.Workers.runAll(jobs);
		}
		// New ones laid out here (fonts aren't safe on several threads).
		for (int k = 0; k < count; k++) {
			if (runs[k] == null) {
				int i = order[k];
				runs[k] = tags.look[i].runs(tags.way[i], tags.y[i], background, font);
			}
		}
		gpu.captureMany(count, runs, order, tags.pose, tags.light);
	}

	private static void keptRuns(Tags tags, int[] order, GlyphRuns.Run[][] runs, int from, int to) {
		for (int k = from; k < to; k++) {
			int i = order[k];
			runs[k] = tags.look[i].kept(tags.way[i], tags.y[i], background);
		}
	}
}
