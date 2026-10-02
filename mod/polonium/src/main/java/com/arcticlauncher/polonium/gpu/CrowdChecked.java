package com.arcticlauncher.polonium.gpu;

/**
 * A player's render state, checked against its recipe while it was made (on
 * the helper threads, see {@link Crowd#precheck}): the recipe it still
 * matches, and for which frame. Added to AvatarRenderState by a mixin.
 */
public interface CrowdChecked {
	Object polonium$checked();

	long polonium$checkedFrame();

	void polonium$checked(Object recipe, long frame);
}
