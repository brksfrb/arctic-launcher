package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.feature.ScreenshotFiles;
import com.mojang.blaze3d.platform.NativeImage;
import java.io.File;
import java.io.IOException;
import net.minecraft.client.Screenshot;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Vanilla already hands the captured NativeImage to an I/O worker; publish its PNG atomically there. */
@Mixin(Screenshot.class)
abstract class ScreenshotMixin {
	//#if MC >= 26.2
	@Redirect(method = "lambda$grab$3", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;writeToFile(Ljava/io/File;)V"))
	//#elif MC >= 26.1
	@Redirect(method = "lambda$grab$1", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;writeToFile(Ljava/io/File;)V"))
	//#elif MC >= 1.21.5
	@Redirect(method = "method_22691", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;writeToFile(Ljava/io/File;)V"))
	//#elif MC >= 1.14
	@Redirect(method = "method_1661", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;writeToFile(Ljava/io/File;)V"))
	//#else
	@Redirect(method = "lambda$grab$1", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;writeToFile(Ljava/io/File;)V"))
	//#endif
	private static void arctic$write(NativeImage image, File file) throws IOException {
		ScreenshotFiles.write(file, image::writeToFile);
	}

	@Inject(method = "getFile", at = @At("RETURN"), cancellable = true)
	private static void arctic$reserve(File directory, CallbackInfoReturnable<File> cir) {
		cir.setReturnValue(ScreenshotFiles.reserve(cir.getReturnValue()));
	}
}
