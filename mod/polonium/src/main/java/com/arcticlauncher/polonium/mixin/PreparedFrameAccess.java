//#if MC >= 26.2
package com.arcticlauncher.polonium.mixin;

import java.util.List;
import net.minecraft.client.renderer.feature.FeatureRenderDispatcher;
import net.minecraft.client.renderer.feature.submit.SubmitNode;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(FeatureRenderDispatcher.PreparedFrame.class)
public interface PreparedFrameAccess {
	@Accessor("allSubmits")
	List<SubmitNode> polonium$allSubmits();
}
//#endif
