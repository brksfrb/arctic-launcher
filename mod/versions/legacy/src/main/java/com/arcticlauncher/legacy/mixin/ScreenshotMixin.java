package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.feature.ScreenshotFiles;
import com.arcticlauncher.legacy.LegacyScreenshots;
import java.awt.image.RenderedImage;
import java.io.File;
import java.io.IOException;
import javax.imageio.ImageIO;
import net.minecraft.client.util.ScreenshotUtils;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Keep vanilla's GL readback, flip/crop and detached BufferedImage; move only PNG encoding and I/O. */
@Mixin(ScreenshotUtils.class)
abstract class ScreenshotMixin {
	//#if MC >= 1.9
	@Redirect(method = "method_12154", at = @At(value = "INVOKE", target = "Ljavax/imageio/ImageIO;write(Ljava/awt/image/RenderedImage;Ljava/lang/String;Ljava/io/File;)Z"))
	//#else
	@Redirect(method = "saveScreenshot(Ljava/io/File;Ljava/lang/String;IILnet/minecraft/client/gl/Framebuffer;)Lnet/minecraft/text/Text;", at = @At(value = "INVOKE", target = "Ljavax/imageio/ImageIO;write(Ljava/awt/image/RenderedImage;Ljava/lang/String;Ljava/io/File;)Z"))
	//#endif
	private static boolean arctic$write(RenderedImage image, String format, File file) throws IOException {
		LegacyScreenshots.write(file, temporary -> {
			if (!ImageIO.write(image, format, temporary)) {
				throw new IOException("No PNG writer available");
			}
		});
		return true;
	}

	@Inject(method = "getScreenshotFile", at = @At("RETURN"), cancellable = true)
	private static void arctic$reserve(File directory, CallbackInfoReturnable<File> cir) {
		cir.setReturnValue(ScreenshotFiles.reserve(cir.getReturnValue()));
	}
}
