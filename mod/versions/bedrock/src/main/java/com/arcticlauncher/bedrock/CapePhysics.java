package com.arcticlauncher.bedrock;

/** Java 26.2 cape motion in model coordinates. Inputs come from a world tick,
 * never from wall-clock render deltas; seeking a replay can reset and resample
 * the same tick sequence. Body crouch/emote poses are applied by the caller. */
public final class CapePhysics {
    private static final float DEG=(float)(Math.PI/180);
    private static final double SIN_SCALE=10430.378350470453;
    private static final float[] SIN=new float[65536];
    static {for(int i=0;i<SIN.length;i++)SIN[i]=(float)Math.sin(i/SIN_SCALE);}
    private static float sin(double v) {return SIN[(int)((long)(v*SIN_SCALE)&65535)];}
    private static float cos(double v) {return SIN[(int)((long)(v*SIN_SCALE+16384)&65535)];}
    public static final class Motion {
        /** Interpolated chasing/cloak position minus the player's position. */
        public double[] delta;
        public float bodyYaw,walkDistance,bob,fallFlyingScale;
        public void validate() {
            if(delta==null || delta.length!=3) throw new IllegalArgumentException("invalid cape displacement");
            for(double v:delta) if(!Double.isFinite(v)||Math.abs(v)>64_000_000) throw new IllegalArgumentException("invalid cape displacement");
            for(float v:new float[]{bodyYaw,walkDistance,bob,fallFlyingScale})
                if(!Float.isFinite(v)||Math.abs(v)>1e9f) throw new IllegalArgumentException("invalid cape motion");
            if(bob<0||bob>1||fallFlyingScale<0||fallFlyingScale>1) throw new IllegalArgumentException("invalid cape motion scale");
        }
        boolean same(Motion other) {
            return other!=null&&java.util.Arrays.equals(delta,other.delta)&&Float.floatToIntBits(bodyYaw)==Float.floatToIntBits(other.bodyYaw)
                &&Float.floatToIntBits(walkDistance)==Float.floatToIntBits(other.walkDistance)&&Float.floatToIntBits(bob)==Float.floatToIntBits(other.bob)
                &&Float.floatToIntBits(fallFlyingScale)==Float.floatToIntBits(other.fallFlyingScale);
        }
    }
    /** Full native filter endpoints for a single completed tick. Keeping the
     * positions (rather than rounded displacement endpoints) preserves the
     * original interpolation/subtraction order at another frame rate. */
    public static final class Snapshot {
        public long tick,flightTicks;
        public double[] position,oldPosition,cloak,oldCloak;
        /** Current/previous native body yaw. */
        public float[] bodyYaw;
        public float walk,oldWalk,bob,oldBob;
        public JavaWingPhysics.Snapshot wingState;
        public void validate() {
            if(tick<0||tick>1_000_000_000_000L||flightTicks<0||flightTicks>0xffffffffL)throw new IllegalArgumentException("invalid cape tick state");
            for(double[] value:new double[][]{position,oldPosition,cloak,oldCloak}) {
                if(value==null||value.length!=3)throw new IllegalArgumentException("invalid cape filter position");
                for(double v:value)if(!Double.isFinite(v)||Math.abs(v)>32_000_000)throw new IllegalArgumentException("invalid cape filter position");
            }
            if(bodyYaw==null||bodyYaw.length!=2)throw new IllegalArgumentException("invalid cape yaw endpoints");
            for(float v:bodyYaw)if(!Float.isFinite(v)||Math.abs(v)>360_000)throw new IllegalArgumentException("invalid cape yaw endpoints");
            for(float v:new float[]{walk,oldWalk,bob,oldBob})if(!Float.isFinite(v)||Math.abs(v)>1e9f)throw new IllegalArgumentException("invalid cape filter phase");
            if(bob<0||bob>1||oldBob<0||oldBob>1)throw new IllegalArgumentException("invalid cape bob endpoints");
            // moveCloak snaps the old/current cloak on a jumping axis while
            // the actor's previous origin can remain across the world span.
            for(int i=0;i<3;i++)if(Math.abs(cloak[i]-position[i])>64_000_000||Math.abs(oldCloak[i]-oldPosition[i])>64_000_000)throw new IllegalArgumentException("invalid cape filter displacement");
            if(wingState!=null) {wingState.validate();if(wingState.tick!=tick)throw new IllegalArgumentException("wing filter belongs to another completed tick");}
        }
        public Motion sample(float partial) {
            validate();if(!Float.isFinite(partial)||partial<0||partial>1)throw new IllegalArgumentException("invalid recorded cape fraction");
            Motion motion=new Motion();motion.delta=new double[3];
            for(int i=0;i<3;i++)motion.delta[i]=oldCloak[i]+(double)partial*(cloak[i]-oldCloak[i])
                -(oldPosition[i]+(double)partial*(position[i]-oldPosition[i]));
            // Mth.rotLerp / wrapDegrees: signed remainder first. Avoid the
            // extra +180/-180 round trip for small float endpoint changes.
            float turn=(bodyYaw[0]-bodyYaw[1])%360;if(turn>=180)turn-=360;if(turn< -180)turn+=360;
            motion.bodyYaw=bodyYaw[1]+partial*turn;
            motion.walkDistance=oldWalk+partial*(walk-oldWalk);motion.bob=oldBob+partial*(bob-oldBob);
            float flight=(float)flightTicks+partial;motion.fallFlyingScale=flightTicks==0?0:Math.min(1,flight*flight/100);
            motion.validate();return motion;
        }
        boolean same(Snapshot other) {
            return tick==other.tick&&flightTicks==other.flightTicks&&java.util.Arrays.equals(position,other.position)
                &&java.util.Arrays.equals(oldPosition,other.oldPosition)&&java.util.Arrays.equals(cloak,other.cloak)&&java.util.Arrays.equals(oldCloak,other.oldCloak)
                &&java.util.Arrays.equals(bodyYaw,other.bodyYaw)&&Float.compare(walk,other.walk)==0&&Float.compare(oldWalk,other.oldWalk)==0
                &&Float.compare(bob,other.bob)==0&&Float.compare(oldBob,other.oldBob)==0
                &&(wingState==null?other.wingState==null:wingState.same(other.wingState));
        }
        void write(java.io.DataOutputStream out) throws java.io.IOException {
            write(out,false);
        }
        void write(java.io.DataOutputStream out,boolean withWings) throws java.io.IOException {
            for(double[] value:new double[][]{position,oldPosition,cloak,oldCloak})for(double v:value)out.writeDouble(v);
            for(float v:bodyYaw)out.writeFloat(v);
            out.writeFloat(walk);out.writeFloat(oldWalk);out.writeFloat(bob);out.writeFloat(oldBob);out.writeInt((int)flightTicks);
            if(withWings) {out.writeBoolean(wingState!=null);if(wingState!=null)wingState.write(out);}
        }
        static Snapshot read(java.io.DataInputStream input,long tick) throws java.io.IOException {
            return read(input,tick,false);
        }
        static Snapshot read(java.io.DataInputStream input,long tick,boolean withWings) throws java.io.IOException {
            Snapshot state=new Snapshot();state.tick=tick;double[][] p=new double[4][3];
            for(double[] value:p)for(int i=0;i<3;i++)value[i]=input.readDouble();
            state.position=p[0];state.oldPosition=p[1];state.cloak=p[2];state.oldCloak=p[3];state.bodyYaw=new float[]{input.readFloat(),input.readFloat()};
            state.walk=input.readFloat();state.oldWalk=input.readFloat();state.bob=input.readFloat();state.oldBob=input.readFloat();state.flightTicks=input.readInt()&0xffffffffL;
            if(withWings) {int present=input.readUnsignedByte();if(present>1)throw new java.io.IOException("invalid recorded wing presence");
                if(present==1)state.wingState=JavaWingPhysics.Snapshot.read(input,tick);}
            state.validate();return state;
        }
    }
    /** {pitch, roll, yaw}, degrees, in the Java cape's X-Z-Y order. */
    public static float[] angles(Motion motion) {
        if(motion==null) return new float[]{6,0,180};
        motion.validate();
        double sine=sin(motion.bodyYaw*DEG),cosine=-cos(motion.bodyYaw*DEG);
        float lift=clamp((float)motion.delta[1]*10,-6,32);
        float lean=clamp((float)(motion.delta[0]*sine+motion.delta[2]*cosine)*100*(1-motion.fallFlyingScale),0,150);
        float side=clamp((float)(motion.delta[0]*cosine-motion.delta[2]*sine)*100,-20,20);
        lift+=sin(motion.walkDistance*6)*32*motion.bob;
        return new float[]{6+lean/2+lift,side/2,180-side/2};
    }
    private static float clamp(float v,float min,float max) { return Math.max(min,Math.min(max,v)); }

    /** The chasing-position filter used when Bedrock has no Java cloak state.
     * Call once per actual game tick (20 Hz), then sample with its partial tick.
     * Do not advance this from a 30/60/144 Hz rendering callback. */
    public static final class Chaser {
        private double[] position,oldPosition,cloak,oldCloak;
        public void reset(double x,double y,double z) {
            check(x,y,z);position=new double[]{x,y,z};oldPosition=position.clone();
            cloak=position.clone();oldCloak=position.clone();
        }
        public void tick(double x,double y,double z) {
            check(x,y,z);
            if(position==null) {reset(x,y,z);return;}
            oldPosition=position;position=new double[]{x,y,z};oldCloak=cloak.clone();
            for(int i=0;i<3;i++) {
                double d=position[i]-cloak[i];
                if(Math.abs(d)>10) oldCloak[i]=cloak[i]=position[i];
                else cloak[i]+=d*.25;
            }
        }
        public double[] delta(float partial) {
            if(position==null || !Float.isFinite(partial)||partial<0||partial>1)
                throw new IllegalArgumentException("cape chaser needs a tick and a partial in [0,1]");
            double[] delta=new double[3];
            for(int i=0;i<3;i++) delta[i]=oldCloak[i]+partial*(cloak[i]-oldCloak[i])
                    -(oldPosition[i]+partial*(position[i]-oldPosition[i]));
            return delta;
        }
        private static void check(double x,double y,double z) {
            for(double v:new double[]{x,y,z}) if(!Double.isFinite(v)||Math.abs(v)>32_000_000)
                throw new IllegalArgumentException("invalid cape position");
        }
    }
}
