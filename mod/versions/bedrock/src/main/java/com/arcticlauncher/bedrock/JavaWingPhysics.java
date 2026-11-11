package com.arcticlauncher.bedrock;

import java.io.*;
import java.util.Arrays;

/** Render sampling of Java 26.2 ElytraAnimationState's completed tick endpoints.
 * The native tick owner advances the filter; rendering/seek never advances it. */
final class JavaWingPhysics {
    private JavaWingPhysics() {}
    static final class Motion {
        final float x,y,z;
        final boolean crouching;
        Motion(float x,float y,float z,boolean crouching) {
            angles(new float[]{x,y,z});this.x=x;this.y=y;this.z=z;this.crouching=crouching;
        }
    }
    static final class Snapshot {
        public long tick,firstTick;
        public float[] previous,current;
        public boolean crouching;
        void validate() {
            if(tick<0||tick>1_000_000_000_000L||firstTick<0||firstTick>tick)
                throw new IllegalArgumentException("invalid Java wing tick origin");
            angles(previous);angles(current);
        }
        Motion sample(float partial) {
            validate();if(!Float.isFinite(partial)||partial<0||partial>1)
                throw new IllegalArgumentException("invalid Java wing tick fraction");
            return new Motion(previous[0]+partial*(current[0]-previous[0]),
                previous[1]+partial*(current[1]-previous[1]),previous[2]+partial*(current[2]-previous[2]),crouching);
        }
        boolean same(Snapshot other) {
            return other!=null&&tick==other.tick&&firstTick==other.firstTick&&crouching==other.crouching
                &&Arrays.equals(previous,other.previous)&&Arrays.equals(current,other.current);
        }
        void write(DataOutput out) throws IOException {
            validate();out.writeLong(firstTick);for(float v:previous)out.writeFloat(v);
            for(float v:current)out.writeFloat(v);out.writeBoolean(crouching);
        }
        static Snapshot read(DataInput in,long tick) throws IOException {
            Snapshot value=new Snapshot();value.tick=tick;value.firstTick=in.readLong();
            value.previous=new float[3];value.current=new float[3];
            for(int i=0;i<3;i++)value.previous[i]=in.readFloat();for(int i=0;i<3;i++)value.current[i]=in.readFloat();
            int crouching=in.readUnsignedByte();if(crouching>1)throw new IOException("invalid recorded wing crouch state");
            value.crouching=crouching==1;value.validate();return value;
        }
    }
    private static void angles(float[] values) {
        if(values==null||values.length!=3)throw new IllegalArgumentException("invalid Java wing rotation count");
        for(float v:values)if(!Float.isFinite(v)||Math.abs(v)>(float)Math.PI)
            throw new IllegalArgumentException("invalid Java wing rotation");
    }
}
