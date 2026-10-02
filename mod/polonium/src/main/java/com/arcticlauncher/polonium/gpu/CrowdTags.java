package com.arcticlauncher.polonium.gpu;

import com.arcticlauncher.polonium.NameTagCache;
import com.mojang.blaze3d.vertex.PoseStack;
import it.unimi.dsi.fastutil.ints.IntArrays;
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

/**
 * The crowd's name tags (see {@link Crowd}): worked out as the game does
 * ({@code EntityRenderer.submitNameDisplay}, {@code SubmitNodeCollection.submitNameTag})
 * but kept in two lists, one per way tags are drawn, each handed to the game
 * as a single tag. When the name tag renderer gets to that tag it draws the
 * whole list on the GPU ({@link GpuText}), see-through ones back to front as
 * the game sorts them. Thousands of tags then skip the game's per-tag
 * submits, sorting and grouping. Render thread only.
 */
public final class CrowdTags {
	/** Stand-ins for the lists, as the text of the tag handed to the game. */
	private static final Component NORMAL = Component.empty();
	private static final Component SEE_THROUGH = Component.empty();
	/** As the game: a see-through tag's shadow color, and the solid pass's light. */
	private static final int TAG_COLOR = -2130706433;
	private static final PoseStack STACK = new PoseStack();
	private static final Matrix4f POSE = new Matrix4f();

	private static final Tags normal = new Tags();
	private static final Tags seeThrough = new Tags();
	private static int background;

	private CrowdTags() {}

	/** One list of tags, as arrays (thousands a frame). */
	private static final class Tags {
		int count;
		Component[] text = new Component[256];
		float[] x = new float[256];
		float[] y = new float[256];
		int[] color = new int[256];
		int[] background = new int[256];
		int[] light = new int[256];
		float[] distance = new float[256];
		float[] pose = new float[256 * 16];
		int farthest = -1;

		void add(Component text, float x, float y, int color, int background, int light, Matrix4f pose) {
			int i = count++;
			if (i == this.text.length) {
				int size = i * 2;
				this.text = Arrays.copyOf(this.text, size);
				this.x = Arrays.copyOf(this.x, size);
				this.y = Arrays.copyOf(this.y, size);
				this.color = Arrays.copyOf(this.color, size);
				this.background = Arrays.copyOf(this.background, size);
				this.light = Arrays.copyOf(this.light, size);
				this.distance = Arrays.copyOf(this.distance, size);
				this.pose = Arrays.copyOf(this.pose, size * 16);
			}
			this.text[i] = text;
			this.x[i] = x;
			this.y[i] = y;
			this.color[i] = color;
			this.background[i] = background;
			this.light[i] = light;
			pose.get(this.pose, i * 16);
			float d = pose.m30() * pose.m30() + pose.m31() * pose.m31() + pose.m32() * pose.m32();
			distance[i] = d;
			if (farthest < 0 || d > distance[farthest]) {
				farthest = i;
			}
		}

		void clear() {
			Arrays.fill(text, 0, count, null);
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
		background = ARGB.color(Minecraft.getInstance().gameRenderer.gameRenderState().optionsRenderState.getBackgroundOpacity(0.25F), -16777216);
	}

	/** A player's score line and name, as {@code AvatarRenderer.submitNameDisplay} places them. */
	static void nameDisplay(AvatarRenderState state, PoseStack poseStack, CameraRenderState camera) {
		int offset = state.showExtraEars ? -10 : 0;
		STACK.last().set(poseStack.last());
		if (state.scoreText != null) {
			tag(state.nameTagAttachment, offset, state.scoreText, !state.isDiscrete, state.lightCoords, camera);
			STACK.translate(0.0F, 9.0F * 1.15F * 0.025F, 0.0F);
		}
		if (state.nameTag != null) {
			tag(state.nameTagAttachment, offset, state.nameTag, !state.isDiscrete, state.lightCoords, camera);
		}
	}

	private static void tag(Vec3 attachment, int offset, Component name, boolean seeThroughToo, int light, CameraRenderState camera) {
		if (attachment == null) {
			return;
		}
		STACK.pushPose();
		STACK.translate(attachment.x, attachment.y + 0.5, attachment.z);
		STACK.mulPose(camera.orientation);
		STACK.scale(0.025F, -0.025F, 0.025F);
		Matrix4f pose = STACK.last().pose();
		float x = -NameTagCache.width(name, Minecraft.getInstance().font) / 2.0F;
		if (seeThroughToo) {
			normal.add(name, x, offset, -1, 0, LightCoordsUtil.lightCoordsWithEmission(light, 2), pose);
			seeThrough.add(name, x, offset, TAG_COLOR, background, light, pose);
		} else {
			normal.add(name, x, offset, TAG_COLOR, background, light, pose);
		}
		STACK.popPose();
	}

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
	 * The name tag renderer got to a list's stand-in: every tag in it, laid
	 * out as the game lays out tags (kept, see {@link NameTagCache}) and drawn
	 * on the GPU; see-through tags back to front.
	 */
	public static void draw(Component marker, GpuText gpu, Font font) {
		Tags tags = marker == NORMAL ? normal : seeThrough;
		Font.DisplayMode mode = marker == NORMAL ? Font.DisplayMode.NORMAL : Font.DisplayMode.SEE_THROUGH;
		int[] order = new int[tags.count];
		for (int i = 0; i < order.length; i++) {
			order[i] = i;
		}
		if (mode == Font.DisplayMode.SEE_THROUGH) {
			float[] distance = tags.distance;
			IntArrays.unstableSort(order, (a, b) -> Float.compare(distance[b], distance[a]));
		}
		for (int i : order) {
			Component text = tags.text[i];
			float x = tags.x[i];
			float y = tags.y[i];
			int color = tags.color[i];
			int background = tags.background[i];
			Font.PreparedText layout = NameTagCache.get(text, x, y, color, background,
					() -> font.prepareText(text.getVisualOrderText(), x, y, color, false, false, background));
			gpu.capture(layout, mode, POSE.set(tags.pose, i * 16), tags.light[i]);
		}
	}
}
