package com.arcticlauncher.polonium.gpu;

import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Collection;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Function;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.ModelPart;
import org.jspecify.annotations.Nullable;
import sun.misc.Unsafe;

/**
 * Private copies of a model for the helper threads. The game poses a model
 * ({@code setupAnim}) by writing its parts, and one model object serves every
 * entity, so posing could only happen on the render thread, one entity at a
 * time. A copy has its own parts (same tree, same starting poses) and shares
 * everything else, so each thread can pose entities with the game's own code
 * at the same time.
 *
 * Only models made of parts, lists and maps of parts, plain values and
 * functions are copied (players, armor, most mobs' simple models); anything
 * holding other objects might keep posing state in them, so it stays on the
 * render thread.
 */
final class ModelCopies {
	/** One model's copy, with its parts in the mesh's order. */
	static final class Copy {
		final Model<?> model;
		final ModelPart[] parts;

		private Copy(Model<?> model, ModelPart[] parts) {
			this.model = model;
			this.parts = parts;
		}
	}

	private static final Map<Class<?>, Boolean> COPYABLE = new ConcurrentHashMap<>();
	private static final Map<Class<?>, List<Field>> FIELDS = new ConcurrentHashMap<>();
	private static final @Nullable Unsafe UNSAFE = unsafe();
	/** Each thread's copies, by original model (held weakly: models are replaced on resource reloads). */
	private static final ThreadLocal<Map<Model<?>, Copy>> COPIES = ThreadLocal.withInitial(java.util.WeakHashMap::new);

	private ModelCopies() {}

	/** Models checked so far (render thread): whether each can be copied. */
	private static final Map<Model<?>, Boolean> CHECKED = new java.util.WeakHashMap<>();

	/**
	 * Whether this model can be copied: its kind holds only copyable fields,
	 * and lists and maps whose element types aren't declared (Fabric API adds
	 * a raw name-to-part map, for one) hold only parts in this model.
	 * Render thread.
	 */
	static boolean copyable(Model<?> model) {
		if (UNSAFE == null || !COPYABLE.computeIfAbsent(model.getClass(), ModelCopies::check)) {
			return false;
		}
		return CHECKED.computeIfAbsent(model, ModelCopies::holdsOnlyParts);
	}

	private static boolean holdsOnlyParts(Model<?> model) {
		try {
			for (Field field : fields(model.getClass())) {
				Object value = field.get(model);
				Collection<?> contents = value instanceof List<?> list ? list : value instanceof Map<?, ?> map ? map.values() : null;
				if (contents != null) {
					for (Object o : contents) {
						if (o != null && !(o instanceof ModelPart)) {
							org.slf4j.LoggerFactory.getLogger("Polonium").info("Polonium: {} is posed on the render thread ({} holds a {})",
									model.getClass().getSimpleName(), field.getName(), o.getClass().getSimpleName());
							return false;
						}
					}
				}
			}
			return true;
		} catch (ReflectiveOperationException | RuntimeException e) {
			return false;
		}
	}

	/** This thread's copy of {@code model} ({@code meshParts}: the original's parts in mesh order). */
	static Copy forThisThread(Model<?> model, ModelPart[] meshParts) {
		Map<Model<?>, Copy> copies = COPIES.get();
		Copy copy = copies.get(model);
		if (copy == null) {
			copy = copy(model, meshParts);
			copies.put(model, copy);
		}
		return copy;
	}

	private static Copy copy(Model<?> original, ModelPart[] meshParts) {
		try {
			Map<ModelPart, ModelPart> parts = new IdentityHashMap<>();
			Model<?> copy = (Model<?>) UNSAFE.allocateInstance(original.getClass());
			for (Field field : fields(original.getClass())) {
				field.set(copy, mapped(field.get(original), parts));
			}
			ModelPart[] ordered = new ModelPart[meshParts.length];
			for (int i = 0; i < meshParts.length; i++) {
				ordered[i] = part(meshParts[i], parts);
			}
			return new Copy(copy, ordered);
		} catch (ReflectiveOperationException e) {
			throw new IllegalStateException("can't copy " + original.getClass().getName(), e);
		}
	}

	/** A field's value in the copy: parts (and lists and maps of them) copied, the rest shared. */
	@SuppressWarnings("unchecked")
	private static Object mapped(Object value, Map<ModelPart, ModelPart> parts) throws ReflectiveOperationException {
		if (value instanceof ModelPart part) {
			return part(part, parts);
		}
		if (value instanceof List<?> list) {
			List<Object> out = new ArrayList<>(list.size());
			for (Object o : list) {
				out.add(mapped(o, parts));
			}
			return out;
		}
		if (value instanceof Map<?, ?> map) {
			Map<Object, Object> out = new LinkedHashMap<>();
			for (Map.Entry<?, ?> e : map.entrySet()) {
				out.put(e.getKey(), mapped(e.getValue(), parts));
			}
			return out;
		}
		return value;
	}

	/** A part's copy (its children too), made once per part. */
	private static ModelPart part(ModelPart original, Map<ModelPart, ModelPart> parts) throws ReflectiveOperationException {
		ModelPart copy = parts.get(original);
		if (copy != null) {
			return copy;
		}
		copy = (ModelPart) UNSAFE.allocateInstance(ModelPart.class);
		parts.put(original, copy);
		for (Field field : fields(ModelPart.class)) {
			Object value = field.get(original);
			field.set(copy, value instanceof Map<?, ?> ? mapped(value, parts) : value);
		}
		return copy;
	}

	/** Every instance field of the class and its superclasses, made writable. */
	private static List<Field> fields(Class<?> type) {
		return FIELDS.computeIfAbsent(type, t -> {
			List<Field> out = new ArrayList<>();
			for (Class<?> c = t; c != null && c != Object.class; c = c.getSuperclass()) {
				for (Field field : c.getDeclaredFields()) {
					if (!Modifier.isStatic(field.getModifiers())) {
						field.setAccessible(true);
						out.add(field);
					}
				}
			}
			return out;
		});
	}

	/** Only parts, lists of parts, maps to parts, plain values, strings, enums and functions. */
	private static boolean check(Class<?> type) {
		try {
			for (Field field : fields(type)) {
				Class<?> t = field.getType();
				boolean plain = t.isPrimitive() || t == String.class || t.isEnum() || t == ModelPart.class || t == Function.class
						|| (t == List.class && partsOrUndeclared(typeArgument(field, 0)))
						|| (t == Map.class && partsOrUndeclared(typeArgument(field, 1)));
				if (!plain) {
					org.slf4j.LoggerFactory.getLogger("Polonium").info("Polonium: {} is posed on the render thread ({} {} can't be copied)",
							type.getSimpleName(), field.getGenericType().getTypeName(), field.getName());
					return false;
				}
			}
			return true;
		} catch (RuntimeException e) {
			return false;
		}
	}

	/** Declared as parts, or not declared (raw): the model's own contents are checked then. */
	private static boolean partsOrUndeclared(java.lang.reflect.@Nullable Type type) {
		return type == null || type == ModelPart.class;
	}

	private static java.lang.reflect.@Nullable Type typeArgument(Field field, int index) {
		if (field.getGenericType() instanceof java.lang.reflect.ParameterizedType p && p.getActualTypeArguments().length > index) {
			return p.getActualTypeArguments()[index];
		}
		return null;
	}

	private static @Nullable Unsafe unsafe() {
		try {
			Field f = Unsafe.class.getDeclaredField("theUnsafe");
			f.setAccessible(true);
			return (Unsafe) f.get(null);
		} catch (ReflectiveOperationException | RuntimeException e) {
			return null;
		}
	}
}
