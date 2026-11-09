package com.arcticlauncher.mod;
//#if MC >= 1.15
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.reflect.Field;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.block.state.BlockState;
//#if MC >= 26.1
import net.minecraft.client.renderer.block.BlockAndTintGetter;
//#else
import net.minecraft.world.level.BlockAndTintGetter;
//#endif
/** Optional Sodium bridge. Resolve handles once; no Sodium dependency is loaded in vanilla. */
public final class GlassSodium {
	private static final ClassValue<MethodHandle> WORLD = new ClassValue<MethodHandle>() {
		@Override protected MethodHandle computeValue(Class<?> type) {
			for (Class<?> c = type; c != null; c = c.getSuperclass()) for (Field field : c.getDeclaredFields()) {
				if (BlockAndTintGetter.class.isAssignableFrom(field.getType())) {
					try { field.setAccessible(true); return MethodHandles.lookup().unreflectGetter(field); }
					catch (IllegalAccessException e) { throw new IllegalStateException("Sodium CTM world access", e); }
				}
			}
			throw new IllegalStateException("Sodium CTM cannot find the meshing world in " + type.getName());
		}
	};
	private static final ClassValue<ContextAccess> CONTEXT = new ClassValue<ContextAccess>() {
		@Override protected ContextAccess computeValue(Class<?> type) { return new ContextAccess(type); }
	};
	private GlassSodium() {}
	public static void renderer(Object renderer, BlockState state, BlockPos pos) {
		if (!GlassCtm.enabled()) { GlassCtm.end(); return; }
		try { GlassCtm.begin((BlockAndTintGetter) WORLD.get(renderer.getClass()).invoke(renderer), state, pos); }
		catch (Throwable e) { throw new IllegalStateException("Sodium CTM context", e); }
	}
	public static void context(Object context) {
		if (!GlassCtm.enabled()) { GlassCtm.end(); return; }
		ContextAccess access = CONTEXT.get(context.getClass());
		try { GlassCtm.begin((BlockAndTintGetter) access.world.invoke(context), (BlockState) access.state.invoke(context), (BlockPos) access.pos.invoke(context)); }
		catch (Throwable e) { throw new IllegalStateException("Sodium CTM context", e); }
	}
	private static final class ContextAccess {
		final MethodHandle world, state, pos;
		ContextAccess(Class<?> type) {
			try {
				world = MethodHandles.publicLookup().unreflect(type.getMethod("world"));
				state = MethodHandles.publicLookup().unreflect(type.getMethod("state"));
				pos = MethodHandles.publicLookup().unreflect(type.getMethod("pos"));
			} catch (ReflectiveOperationException e) { throw new IllegalStateException("Sodium CTM access", e); }
		}
	}
}
//#endif
