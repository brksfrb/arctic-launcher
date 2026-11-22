package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.feature.ScreenshotFiles;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.mojang.blaze3d.platform.NativeImage;
import java.io.File;
import java.io.IOException;
import net.minecraft.client.Screenshot;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Vanilla already hands the captured NativeImage to an I/O worker; publish its PNG atomically there. */
@Mixin(Screenshot.class)
abstract class ScreenshotMixin {
	//#if NEOFORGE
	// NeoForge's screenshot (its grab is _grab) saves in a lambda of its own number (and, on 1.20.1,
	// of Forge's names): whichever lambda writes the file.
	@WrapOperation(method = "/lambda\\$.+/", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;writeToFile(Ljava/io/File;)V"))
	//#elif MC >= 26.2
	@WrapOperation(method = "lambda$grab$3", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;writeToFile(Ljava/io/File;)V"))
	//#elif MC >= 26.1
	@WrapOperation(method = "lambda$grab$1", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;writeToFile(Ljava/io/File;)V"))
	//#elif MC >= 1.21.5
	@WrapOperation(method = "method_22691", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;writeToFile(Ljava/io/File;)V"))
	//#elif MC >= 1.14
	@WrapOperation(method = "method_1661", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;writeToFile(Ljava/io/File;)V"))
	//#else
	@WrapOperation(method = "lambda$grab$1", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;writeToFile(Ljava/io/File;)V"))
	//#endif
	private static void arctic$write(NativeImage image, File file, Operation<Void> original) throws IOException {
		ScreenshotFiles.write(file, temporary -> original.call(image, temporary));
	}

	@Inject(method = "getFile", at = @At("RETURN"), cancellable = true)
	private static void arctic$reserve(File directory, CallbackInfoReturnable<File> cir) {
		cir.setReturnValue(ScreenshotFiles.reserve(cir.getReturnValue()));
	}
}
