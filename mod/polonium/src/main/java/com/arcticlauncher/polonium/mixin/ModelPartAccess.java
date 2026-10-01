package com.arcticlauncher.polonium.mixin;

import java.util.List;
import java.util.Map;
import net.minecraft.client.model.geom.ModelPart;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(ModelPart.class)
public interface ModelPartAccess {
	@Accessor("cubes")
	List<ModelPart.Cube> polonium$cubes();

	@Accessor("children")
	Map<String, ModelPart> polonium$children();
}
