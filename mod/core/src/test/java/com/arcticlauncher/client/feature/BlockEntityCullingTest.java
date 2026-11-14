package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.config.ClientConfig;
import java.util.Random;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

public class BlockEntityCullingTest {
	@AfterEach
	void reset() {
		BlockEntityCulling.shadowPasses(false);
		BlockEntityCulling.end();
	}

	@Test
	void onByDefaultWithTheGamesDistance() {
		ClientConfig c = new ClientConfig();
		assertTrue(c.blockEntityCulling);
		assertTrue(BlockEntityCulling.active(c));
		assertEquals(BlockEntityCulling.GAME_DISTANCE, BlockEntityCulling.distance(c));
		assertEquals(BlockEntityCulling.GAME_DISTANCE, c.blockEntityDistance);
	}

	@Test
	void offDrawsEverything() {
		ClientConfig c = new ClientConfig();
		c.blockEntityCulling = false;
		c.blockEntityDistance = 16;
		BlockEntityCulling.View nothing = (a, b, d, e, f, g) -> false;
		assertFalse(BlockEntityCulling.skip(c, 1000, 64, 1000, 0, 64, 0, nothing));
		assertEquals(BlockEntityCulling.GAME_DISTANCE, BlockEntityCulling.distance(c));
		assertFalse(BlockEntityCulling.skip(null, 1000, 64, 1000, 0, 64, 0, nothing));
	}

	@Test
	void shaderShadowPassesTurnItOff() {
		ClientConfig c = new ClientConfig();
		BlockEntityCulling.shadowPasses(true);
		assertFalse(BlockEntityCulling.active(c));
		assertFalse(BlockEntityCulling.skip(c, 5, 5, 5, 0, 0, 0, (a, b, d, e, f, g) -> false));
	}

	@Test
	void testsABoxOneBlockBiggerThanTheBlock() {
		ClientConfig c = new ClientConfig();
		final double[] seen = new double[6];
		BlockEntityCulling.View view = (minX, minY, minZ, maxX, maxY, maxZ) -> {
			seen[0] = minX;
			seen[1] = minY;
			seen[2] = minZ;
			seen[3] = maxX;
			seen[4] = maxY;
			seen[5] = maxZ;
			return true;
		};
		assertFalse(BlockEntityCulling.skip(c, 10, 64, -3, 0, 64, 0, view));
		// Double chests (the other half), open lids and shulker lids, standing and wall banners reach at most one block out.
		assertEquals(9, seen[0]);
		assertEquals(63, seen[1]);
		assertEquals(-4, seen[2]);
		assertEquals(12, seen[3]);
		assertEquals(66, seen[4]);
		assertEquals(-1, seen[5]);
		assertTrue(BlockEntityCulling.skip(c, 10, 64, -3, 0, 64, 0, (a, b, d, e, f, g) -> false));
	}

	@Test
	void noViewOnlyDistance() {
		ClientConfig c = new ClientConfig();
		assertFalse(BlockEntityCulling.skip(c, 200, 64, 0, 0, 64, 0, null), "the game's own distance check stays the game's");
		c.blockEntityDistance = 32;
		assertEquals(32, BlockEntityCulling.distance(c));
		assertFalse(BlockEntityCulling.skip(c, 31, 64, 0, 0.5, 64.5, 0.5, null));
		assertTrue(BlockEntityCulling.skip(c, 33, 64, 0, 0.5, 64.5, 0.5, null));
		// Out-of-range settings fall back to the game's distance.
		c.blockEntityDistance = 500;
		assertEquals(BlockEntityCulling.GAME_DISTANCE, BlockEntityCulling.distance(c));
		c.blockEntityDistance = 0;
		assertEquals(BlockEntityCulling.GAME_DISTANCE, BlockEntityCulling.distance(c));
	}

	@Test
	void frustumOnlyInsideTheWorldPassAndTheFirstOneWins() {
		Object camera = new Object();
		Object other = new Object();
		BlockEntityCulling.seen(camera);
		assertNull(BlockEntityCulling.frustum(), "outside the world's drawing nothing is known");
		BlockEntityCulling.begin();
		assertNull(BlockEntityCulling.frustum());
		BlockEntityCulling.seen(camera);
		BlockEntityCulling.seen(other);
		assertSame(camera, BlockEntityCulling.frustum());
		BlockEntityCulling.end();
		assertNull(BlockEntityCulling.frustum(), "previews drawn after the world are never culled");
		BlockEntityCulling.begin();
		assertNull(BlockEntityCulling.frustum(), "each frame starts without one");
	}

	/** Whatever part of a block entity's drawing is on screen, it is never skipped. */
	@Test
	void neverSkipsAnythingWhoseDrawingIsInView() {
		ClientConfig c = new ClientConfig();
		Random r = new Random(7);
		int checked = 0;
		for (int i = 0; i < 2000; i++) {
			SceneBench.Camera cam = SceneBench.Camera.random(r);
			for (int k = 0; k < 50; k++) {
				int x = (int) Math.floor(cam.x) + r.nextInt(80) - 40;
				int y = (int) Math.floor(cam.y) + r.nextInt(40) - 20;
				int z = (int) Math.floor(cam.z) + r.nextInt(80) - 40;
				// The drawing: a double chest's other half on any side, an open lid above it.
				int dx = r.nextInt(3) - 1;
				int dz = dx == 0 ? r.nextInt(3) - 1 : 0;
				double[] drawn = {Math.min(x, x + dx) + 1 / 16.0, y, Math.min(z, z + dz) + 1 / 16.0,
						Math.max(x, x + dx) + 15 / 16.0, y + 1.5, Math.max(z, z + dz) + 15 / 16.0};
				boolean visible = cam.sees(drawn[0], drawn[1], drawn[2], drawn[3], drawn[4], drawn[5]);
				if (visible) {
					checked++;
					assertFalse(BlockEntityCulling.skip(c, x, y, z, cam.x, cam.y, cam.z, cam), "skipped a block entity in view");
				}
			}
		}
		assertTrue(checked > 10000);
	}
}
