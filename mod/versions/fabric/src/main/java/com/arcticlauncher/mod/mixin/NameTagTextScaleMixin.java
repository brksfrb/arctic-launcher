package com.arcticlauncher.mod.mixin;

//#if MC >= 26.2 && MC < 26.3
import com.arcticlauncher.mod.ScaledInter;
import com.arcticlauncher.mod.TextDrawScale;
import com.google.common.collect.MapMaker;
import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import java.util.concurrent.ConcurrentMap;
import net.minecraft.client.renderer.feature.NameTagFeatureRenderer;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.MutableComponent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 26.2 draws name tags in their own feature renderer, not in {@code TextFeatureRenderer}: without this their letters
 * never came from a size bucket, so close-up tags were the GUI-size atlas magnified with a linear filter (soft edges).
 *
 * <p>Each tag is drawn with the scale of its own projected size. A laid-out tag is also kept per bucket: mods that keep a
 * tag's laid-out glyphs on its text object between frames (Polarium) would otherwise freeze the letters of whichever
 * bucket the tag was first seen at, so a tag first seen from afar stays soft when it is walked up to. Each bucket's
 * text is its own copy, so such a cache holds one layout per bucket and never lays out again for the same one.
 */
@Mixin(NameTagFeatureRenderer.class)
abstract class NameTagTextScaleMixin {
	/** By text object (identity): the copies of it for buckets 1..3. */
	@Unique private static final ConcurrentMap<Component, Component[]> arctic$copies = new MapMaker().weakKeys().makeMap();

	@ModifyExpressionValue(method = "buildGroup", at = @At(value = "INVOKE", target = "Ljava/util/Iterator;next()Ljava/lang/Object;"))
	private Object arctic$tag(Object next) {
		if (!TextDrawScale.on() || !(next instanceof NameTagFeatureRenderer.Submit tag)) {
			TextDrawScale.restore(0);
			return next;
		}
		// Stays set until the next tag (or the end of the group): the tag is laid out when it is drawn.
		float times = TextDrawScale.pixels(tag.pose(), 0, 0);
		TextDrawScale.set(times);
		int variant = ScaledInter.variant(TextDrawScale.current());
		Component text = tag.text();
		if (variant == 0 || text.getClass() != MutableComponent.class) {
			return next;
		}
		Component[] copies = arctic$copies.computeIfAbsent(text, t -> new Component[3]);
		Component copy = copies[variant - 1];
		if (copy == null) {
			copy = text.copy();
			copies[variant - 1] = copy;
		}
		return new NameTagFeatureRenderer.Submit(tag.pose(), tag.x(), tag.y(), copy, tag.lightCoords(), tag.color(),
				tag.backgroundColor(), tag.displayMode());
	}

	@Inject(method = "buildGroup", at = @At("RETURN"))
	private void arctic$done(CallbackInfo ci) {
		TextDrawScale.restore(0);
	}
}
//#endif
