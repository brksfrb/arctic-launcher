package com.arcticlauncher.polonium.gpu;

/**
 * A feature renderer's GPU side: told which of the game's groups is being
 * prepared, draws its part of a group when the game draws that group, and
 * starts over each frame (see RenderTypeFeatureRendererMixin).
 */
public interface GpuFeature {
	void beginGroup(boolean strictlyOrdered);

	void endGroup();

	void executeGroup(int groupIndex);

	void endFrame();
}
