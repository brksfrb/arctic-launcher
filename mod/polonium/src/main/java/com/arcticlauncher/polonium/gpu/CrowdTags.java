package com.arcticlauncher.polonium.gpu;

import com.arcticlauncher.polonium.GlyphRuns;
import com.arcticlauncher.polonium.NameTagCache;
import com.mojang.blaze3d.vertex.PoseStack;
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
	private static final PoseStack STACK = new PoseStack();
	private static final Matrix4f POSE = new Matrix4f();
	/** This frame's tag turn (to face the camera) and scale, the same for every tag. */
	private static final Matrix4f FACING = new Matrix4f();
	private static final Matrix4f TAG = new Matrix4f();

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

	/** One list of tags, as arrays (thousands a frame). */
	private static final class Tags {
		int count;
		Look[] look = new Look[256];
		byte[] way = new byte[256];
		float[] y = new float[256];
		int[] light = new int[256];
		float[] distance = new float[256];
		float[] pose = new float[256 * 16];
		int farthest = -1;

		void add(Look look, int way, float y, int light, Matrix4f pose) {
			int i = count++;
			if (i == this.look.length) {
				int size = i * 2;
				this.look = Arrays.copyOf(this.look, size);
				this.way = Arrays.copyOf(this.way, size);
				this.y = Arrays.copyOf(this.y, size);
				this.light = Arrays.copyOf(this.light, size);
				this.distance = Arrays.copyOf(this.distance, size);
				this.pose = Arrays.copyOf(this.pose, size * 16);
			}
			this.look[i] = look;
			this.way[i] = (byte) way;
			this.y[i] = y;
			this.light[i] = light;
			pose.get(this.pose, i * 16);
			float d = pose.m30() * pose.m30() + pose.m31() * pose.m31() + pose.m32() * pose.m32();
			distance[i] = d;
			if (farthest < 0 || d > distance[farthest]) {
				farthest = i;
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

	/** A player's score line and name, as {@code AvatarRenderer.submitNameDisplay} places them. */
	static void nameDisplay(AvatarRenderState state, CrowdRecipe recipe, PoseStack poseStack, CameraRenderState camera) {
		int offset = state.showExtraEars ? -10 : 0;
		STACK.last().set(poseStack.last());
		if (state.scoreText != null) {
			recipe.scoreLook = look(recipe.scoreLook, state.scoreText);
			tag(state.nameTagAttachment, offset, recipe.scoreLook, !state.isDiscrete, state.lightCoords, camera);
			STACK.translate(0.0F, 9.0F * 1.15F * 0.025F, 0.0F);
		}
		if (state.nameTag != null) {
			recipe.nameLook = look(recipe.nameLook, state.nameTag);
			tag(state.nameTagAttachment, offset, recipe.nameLook, !state.isDiscrete, state.lightCoords, camera);
		}
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

	private static void tag(@Nullable Vec3 attachment, int offset, Look look, boolean seeThroughToo, int light, CameraRenderState camera) {
		if (attachment == null) {
			return;
		}
		if (!facingReady) {
			// As the game: turn to face the camera, then scale (worked out once, applied to every tag).
			FACING.rotation(camera.orientation).scale(0.025F, -0.025F, 0.025F);
			facingReady = true;
		}
		Matrix4f pose = TAG.set(STACK.last().pose()).translate((float) attachment.x, (float) (attachment.y + 0.5), (float) attachment.z)
				.mul(FACING);
		if (seeThroughToo) {
			normal.add(look, SOLID_PASS, offset, LightCoordsUtil.lightCoordsWithEmission(light, 2), pose);
			seeThrough.add(look, SEE_THROUGH_PASS, offset, light, pose);
		} else {
			normal.add(look, DISCREET, offset, light, pose);
		}
	}

	private static boolean facingReady;

	/** All entities are in: each list goes to the game as one tag, where the farthest of its tags is. */
	static void endSubmits(SubmitNodeStorage storage) {
		SubmitNodeCollection collection = storage.order(0);
		if (normal.count > 0) {
			collection.nameTags.submit(standIn(normal, NORMAL, Font.DisplayMode.NORMAL));
		}
		if (seeThrough.count > 0) {
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
			Crowd.sortFarToNear(order, order.length, tags.distance);
		}
		int count = order.length;
		GlyphRuns.Run[][] runs = new GlyphRuns.Run[count][];
		// Kept runs, on the helper threads (most tags, every frame after their first).
		int parts = count < 256 ? 1 : com.arcticlauncher.polonium.Workers.HELPERS + 1;
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
