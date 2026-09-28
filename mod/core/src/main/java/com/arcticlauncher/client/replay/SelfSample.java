package com.arcticlauncher.client.replay;

import java.io.DataInput;
import java.io.DataOutput;
import java.io.IOException;

/**
 * The recording player, once a tick: servers never send you your own
 * movement, so the recorder notes it. It moves your body in the replay and
 * is the camera for 2D (first person) clips.
 */
public final class SelfSample {
	public static final int SNEAKING = 1;
	public static final int SPRINTING = 1 << 1;
	public static final int SWIMMING = 1 << 2;
	public static final int FLYING_ELYTRA = 1 << 3;
	public static final int USING_ITEM = 1 << 4;
	public static final int ON_GROUND = 1 << 5;
	/** Size of one sample in the file. */
	static final int BYTES = 4 + 8 * 3 + 4 * 4 + 1 + 1 + 1 + 4 + 1 + 4;

	public int time;
	public double x;
	public double y;
	public double z;
	public float yaw;
	public float pitch;
	public float headYaw;
	public float bodyYaw;
	public int flags;
	/** Hotbar slot 0-8. */
	public int slot;
	/** Swung this tick: 0 no, 1 main hand, 2 off hand. */
	public int swing;
	public float health;
	public int food;
	/** The field of view you played with (degrees). */
	public float fov;

	public boolean has(int flag) {
		return (flags & flag) != 0;
	}

	void write(DataOutput out) throws IOException {
		out.writeInt(time);
		out.writeDouble(x);
		out.writeDouble(y);
		out.writeDouble(z);
		out.writeFloat(yaw);
		out.writeFloat(pitch);
		out.writeFloat(headYaw);
		out.writeFloat(bodyYaw);
		out.writeByte(flags);
		out.writeByte(slot);
		out.writeByte(swing);
		out.writeFloat(health);
		out.writeByte(food);
		out.writeFloat(fov);
	}

	static SelfSample read(DataInput in) throws IOException {
		SelfSample s = new SelfSample();
		s.time = in.readInt();
		s.x = in.readDouble();
		s.y = in.readDouble();
		s.z = in.readDouble();
		s.yaw = in.readFloat();
		s.pitch = in.readFloat();
		s.headYaw = in.readFloat();
		s.bodyYaw = in.readFloat();
		s.flags = in.readUnsignedByte();
		s.slot = in.readUnsignedByte();
		s.swing = in.readUnsignedByte();
		s.health = in.readFloat();
		s.food = in.readUnsignedByte();
		s.fov = in.readFloat();
		return s;
	}
}
