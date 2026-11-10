package com.arcticlauncher.mod.mixin;
//#if MC >= 1.19.3
import com.arcticlauncher.mod.GlassSpriteSource;
import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.client.resources.metadata.animation.FrameSize;
import net.minecraft.client.resources.metadata.animation.AnimationMetadataSection;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
@Mixin(SpriteContents.class)
abstract class GlassContentsMixin implements GlassSpriteSource {
	@Shadow @Final private NativeImage originalImage;
	@Shadow @Final private int width;
	@Shadow @Final private int height;
	//#if MC >= 1.21.9
	@Unique private java.util.Optional<AnimationMetadataSection> arctic$animation;
	@Unique private java.util.List<net.minecraft.server.packs.metadata.MetadataSectionType.WithValue<?>> arctic$additional;
	//#if MC >= 1.21.11
	@Unique private java.util.Optional<net.minecraft.client.resources.metadata.texture.TextureMetadataSection> arctic$texture;
	@Inject(method = "<init>(Lnet/minecraft/resources/Identifier;Lnet/minecraft/client/resources/metadata/animation/FrameSize;Lcom/mojang/blaze3d/platform/NativeImage;Ljava/util/Optional;Ljava/util/List;Ljava/util/Optional;)V", at = @At("RETURN"))
	private void arctic$metadata(Identifier id, FrameSize size, NativeImage image, java.util.Optional<AnimationMetadataSection> animation, java.util.List<net.minecraft.server.packs.metadata.MetadataSectionType.WithValue<?>> additional, java.util.Optional<net.minecraft.client.resources.metadata.texture.TextureMetadataSection> texture, CallbackInfo ci) {
		arctic$animation = animation; arctic$additional = additional; arctic$texture = texture;
	}
	//#else
	@Inject(method = "<init>(Lnet/minecraft/resources/Identifier;Lnet/minecraft/client/resources/metadata/animation/FrameSize;Lcom/mojang/blaze3d/platform/NativeImage;Ljava/util/Optional;Ljava/util/List;)V", at = @At("RETURN"))
	private void arctic$metadata(Identifier id, FrameSize size, NativeImage image, java.util.Optional<AnimationMetadataSection> animation, java.util.List<net.minecraft.server.packs.metadata.MetadataSectionType.WithValue<?>> additional, CallbackInfo ci) {
		arctic$animation = animation; arctic$additional = additional;
	}
	//#endif
	//#elif MC >= 1.20.2
	@Shadow @Final private net.minecraft.server.packs.resources.ResourceMetadata metadata;
	//#else
	@Unique private AnimationMetadataSection arctic$animation;
	@Inject(method = "<init>", at = @At("RETURN"))
	private void arctic$metadata(Identifier id, FrameSize size, NativeImage image, AnimationMetadataSection metadata, CallbackInfo ci) { arctic$animation = metadata; }
	//#endif
	@Override public NativeImage arctic$image() { return originalImage; }
	@Override public SpriteContents arctic$variant(Identifier id, NativeImage image) {
		//#if MC >= 1.21.11
		return new SpriteContents(id, new FrameSize(width, height), image, arctic$animation, arctic$additional, arctic$texture);
		//#elif MC >= 1.21.9
		return new SpriteContents(id, new FrameSize(width, height), image, arctic$animation, arctic$additional);
		//#elif MC >= 1.20.2
		return new SpriteContents(id, new FrameSize(width, height), image, metadata);
		//#else
		return new SpriteContents(id, new FrameSize(width, height), image, arctic$animation);
		//#endif
	}
}
//#endif
