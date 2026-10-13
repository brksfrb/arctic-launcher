package com.arcticlauncher.client.packs;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

public class PackVisibilityTest {

	@Test
	void noFabricApiLeavesNormalPacksVisible() {
		assertFalse(PackVisibility.isHidden(new Object()));
	}

	@Test
	void loaderV1HidesInternalResourcesButKeepsOptionalBuiltinPacks() {
		assertTrue(PackVisibility.isHidden(new V1Pack(true)));
		assertFalse(PackVisibility.isHidden(new V1Pack(false)));
	}

	@Test
	void loaderV0HasTheSameVisibilityWithItsOlderHookName() {
		assertTrue(PackVisibility.isHidden(new V0Pack(true)));
		assertFalse(PackVisibility.isHidden(new V0Pack(false)));
	}

	public static final class V1Pack {
		private final boolean hidden;
		V1Pack(boolean hidden) { this.hidden = hidden; }
		public boolean fabric$isHidden() { return hidden; }
	}

	public static final class V0Pack {
		private final boolean hidden;
		V0Pack(boolean hidden) { this.hidden = hidden; }
		public boolean fabric_isHidden() { return hidden; }
	}
}
