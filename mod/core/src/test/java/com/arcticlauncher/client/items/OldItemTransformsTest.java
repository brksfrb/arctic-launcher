package com.arcticlauncher.client.items;

import com.arcticlauncher.client.items.OldItemTransforms.Kind;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.EnumSource;
import static org.junit.jupiter.api.Assertions.*;

/** Ground truth: official 1.7.10 bly.renderItem / bop.renderEquippedItems,
* compared with the COMPLETE 1.8.9 bfn -> bjh draw chain (including preTransform).
* Reference operates in 1.7 quad coordinates; actual operates in baked-model coordinates.
*/
class OldItemTransformsTest {
	@ParameterizedTest @EnumSource(Kind.class)
	void completeFirstAndThirdPersonChains(Kind kind) {
		for (boolean third : new boolean[] {false, true}) {
			for (boolean rotating : new boolean[] {false, true}) {
				for (boolean blocking : new boolean[] {false, true}) {
					Matrix expected = caller(third, blocking);
					reference1710(expected, kind, third, rotating, blocking);
					if (kind != Kind.BLOCK) {
						// U goes max -> min in 1.7; model's NORTH face is z=7.5/16.
						expected.translate(1, 0, 7.5f/16f);
						expected.scale(-1, 1, -1);
					}
					Matrix actual = caller(third, blocking);
					// Flat: RenderItem.preTransform. Block: HeldItemRenderer.renderItem.
					actual.scale(2, 2, 2);
					OldItemTransforms.apply(actual, kind, third, rotating, blocking);
					actual.scale(.5f, .5f, .5f); // RenderItem.renderItem(stack, model)
					actual.translate(-.5f, -.5f, -.5f);
					assertArrayEquals(expected.m, actual.m, 2e-6,
							kind + " third=" + third + " rotate=" + rotating + " block=" + blocking);
				}
			}
		}
	}

	@Test void replacementCullingDependsOnLegacyPoseOnly() {
		for (Kind kind : Kind.values()) {
			assertFalse(OldItemTransforms.mirrored(kind, false));
			assertEquals(kind == Kind.BOW || kind == Kind.TOOL, OldItemTransforms.mirrored(kind, true));
		}
	}

	@Test void blockingArmUses1710YawAndPreservesVanillaWhenOff() {
		float vanilla = -0.5235988f;
		assertEquals(vanilla, OldItemTransforms.blockingArmYaw(vanilla, false));
		Matrix expected = new Matrix();
		expected.rotate(8,0,0,1); expected.rotate(-54,1,0,0);
		Matrix actual = new Matrix();
		actual.rotate(8,0,0,1);
		actual.rotate((float)Math.toDegrees(OldItemTransforms.blockingArmYaw(vanilla,true)),0,1,0);
		actual.rotate(-54,1,0,0);
		assertArrayEquals(expected.m, actual.m, 1e-7);
	}

	@Test void drawnBowMatches1710AtEveryPullStage() {
		for (int ticks : new int[] {0, 1, 5, 10, 15, 20, 40}) {
			float pull = ticks / 20f;
			pull = Math.min(1f, (pull * pull + 2f * pull) / 3f);
			float stretch = 1f + .2f * pull;
			Matrix expected = new Matrix();
			expected.rotate(-335,0,0,1); expected.rotate(-50,0,1,0);
			expected.translate(0,.5f,0); expected.scale(1,1,stretch); expected.translate(0,-.5f,0);
			expected.rotate(50,0,1,0); expected.rotate(335,0,0,1);
			Matrix actual = new Matrix();
			actual.scale(1,1,stretch); // applyBowTransformation's 1.8 stretch
			OldItemTransforms.bowStretch(actual, stretch);
			assertArrayEquals(expected.m, actual.m, 2e-6, "pull ticks=" + ticks);
		}
	}

	@Test void textureFrontAndBackMatch1710IncludingThickness() {
		Matrix actual = new Matrix(); actual.scale(2,2,2);
		OldItemTransforms.apply(actual, Kind.FLAT, false, false, false);
		actual.scale(.5f,.5f,.5f); actual.translate(-.5f,-.5f,-.5f);
		Matrix expected = new Matrix(); reference1710(expected, Kind.FLAT, false, false, false);
		for (float x : new float[] {0,1}) for (float y : new float[] {0,1}) {
			assertArrayEquals(expected.point(1-x,y,0), actual.point(x,y,7.5f/16), 2e-6);
			assertArrayEquals(expected.point(1-x,y,-1f/16), actual.point(x,y,8.5f/16), 2e-6);
		}
	}

	private static Matrix caller(boolean third, boolean blocking) {
		Matrix m = new Matrix();
		if (third) {
			// Arbitrary postRenderArm pose proves cancellation preserves the hand attachment.
			m.translate(-.3125f,.125f,0); m.rotate(-35,1,0,0); m.rotate(12,0,0,1);
			m.translate(-.0625f,.4375f,.0625f);
		} else {
			m.translate(.56f,-.52f,-.72f); m.rotate(45,0,1,0); m.scale(.4f,.4f,.4f);
			if (blocking) { m.translate(-.5f,.2f,0); m.rotate(30,0,1,0); m.rotate(-80,1,0,0); m.rotate(60,0,1,0); }
		}
		return m;
	}

	/** Literal legacy GL chain; no baked-model compensation is allowed in this reference. */
	private static void reference1710(Matrix m, Kind kind, boolean third, boolean rotating, boolean blocking) {
		if (third) {
			switch (kind) {
			case BLOCK:
				m.translate(0,.1875f,-.3125f); m.rotate(20,1,0,0); m.rotate(45,0,1,0); m.scale(-.375f,-.375f,.375f); break;
			case BOW:
				m.translate(0,.125f,.3125f); m.rotate(-20,0,1,0); m.scale(.625f,-.625f,.625f); m.rotate(-100,1,0,0); m.rotate(45,0,1,0); break;
			case TOOL:
				if (rotating) { m.rotate(180,0,0,1); m.translate(0,-.125f,0); }
				if (blocking) { m.translate(.05f,0,-.1f); m.rotate(-50,0,1,0); m.rotate(-10,1,0,0); m.rotate(-60,0,0,1); }
				m.translate(0,.1875f,0); m.scale(.625f,-.625f,.625f); m.rotate(-100,1,0,0); m.rotate(45,0,1,0); break;
			default:
				m.translate(.25f,.1875f,-.1875f); m.scale(.375f,.375f,.375f); m.rotate(60,0,0,1); m.rotate(-90,1,0,0); m.rotate(20,0,0,1);
			}
		} else if (rotating) m.rotate(180,0,1,0);
		if (kind == Kind.BLOCK) {
			m.rotate(90,0,1,0); m.translate(-.5f,-.5f,-.5f);
		} else {
			m.translate(0,-.3f,0); m.scale(1.5f,1.5f,1.5f); m.rotate(50,0,1,0); m.rotate(335,0,0,1); m.translate(-.9375f,-.0625f,0);
		}
	}

	static final class Matrix implements OldItemTransforms.Ops {
		double[] m = identity();
		private static double[] identity() { return new double[] {1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1}; }
		private void multiply(double[] b) {
			double[] out = new double[16];
			for (int r=0;r<4;r++) for (int c=0;c<4;c++) for (int k=0;k<4;k++) out[r*4+c] += m[r*4+k]*b[k*4+c];
			m=out;
		}
		public void translate(float x,float y,float z) { double[] b=identity(); b[3]=x;b[7]=y;b[11]=z;multiply(b); }
		public void scale(float x,float y,float z) { double[] b=identity();b[0]=x;b[5]=y;b[10]=z;multiply(b); }
		public void rotate(float degrees,float x,float y,float z) {
			double c=Math.cos(Math.toRadians(degrees)),s=Math.sin(Math.toRadians(degrees)),t=1-c;
			multiply(new double[] {t*x*x+c,t*x*y-s*z,t*x*z+s*y,0, t*x*y+s*z,t*y*y+c,t*y*z-s*x,0, t*x*z-s*y,t*y*z+s*x,t*z*z+c,0, 0,0,0,1});
		}
		double[] point(float x,float y,float z) {
			return new double[] {m[0]*x+m[1]*y+m[2]*z+m[3],m[4]*x+m[5]*y+m[6]*z+m[7],m[8]*x+m[9]*y+m[10]*z+m[11]};
		}
	}
}
