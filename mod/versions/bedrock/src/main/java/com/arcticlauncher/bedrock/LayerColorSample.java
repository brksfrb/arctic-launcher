package com.arcticlauncher.bedrock;

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.security.*;
import java.util.Arrays;

/** Two independently observed layer colors: unknown (0), absent (1), or an
 * owned draw (2), with optional dithering and named CPU shader inputs.
 * Fingerprints distinguish geometry/texture layouts, not native
 * addresses or a complete material/shader identity. */
final class LayerColorSample {
    static final int CAPE=0,WING=1;
    private final int states;
    private final byte[] identities;
    private final float[] rgba;
    private final byte[] dithering;
    private final ActorShaderInputs[] shaders;
    private LayerColorSample(int states,byte[] identities,float[] rgba,byte[] dithering,ActorShaderInputs[] shaders) {
        if(states<0||states>15||identities.length!=64||rgba.length!=8||dithering.length!=2||shaders.length!=2)throw new IllegalArgumentException("invalid layer color sample");
        for(int role=0;role<2;role++)if(((states>>(role*2))&3)>2||dithering[role]<0||dithering[role]>2
                ||((states>>(role*2))&3)!=2&&(dithering[role]!=0||shaders[role]!=null))throw new IllegalArgumentException("invalid layer color/dithering state");
        for(float value:rgba)if(!Float.isFinite(value)||value<0||value>1)throw new IllegalArgumentException("invalid sampled layer color");
        this.states=states;this.identities=identities.clone();this.rgba=rgba.clone();this.dithering=dithering.clone();
        this.shaders=new ActorShaderInputs[2];for(int role=0;role<2;role++)if(shaders[role]!=null)this.shaders[role]=shaders[role].copy();
    }
    static LayerColorSample observe(WorldFrame.Player player) throws IOException {
        int cape=player.nativeCapeAbsent&&player.nativeCape==null?1:player.nativeCapeDrawColor!=null?2:0;
        int wing=Boolean.FALSE.equals(player.elytraEquipped)?1:player.nativeWingDrawColor!=null?2:0;
        byte[] identities=new byte[64],dithering=new byte[2];float[] rgba=new float[8];
        ActorShaderInputs[] shaders=new ActorShaderInputs[2];
        for(int role=0;role<2;role++)if((role==CAPE?cape:wing)==2) {
            WorldFrame.DrawColor draw=role==CAPE?player.nativeCapeDrawColor:player.nativeWingDrawColor;draw.validate(player,role==WING);
            System.arraycopy(identity(draw.geometry,draw.nativeCape),0,identities,role*32,32);
            float[] applied=draw.rgba;
            Boolean flag=draw.ditheringEnabled;
            ActorShaderInputs appliedShaders=draw.shaderInputs;
            WorldFrame.CapeReplayColor receipt=role==CAPE?player.nativeCapeReplayColor:player.nativeWingReplayColor;
            if(receipt!=null) {
                receipt.validate(player,role==WING);applied=receipt.rgba;
                flag=receipt.ditheringEnabled;
                if(Boolean.TRUE.equals(receipt.shaderInputsApplied)&&receipt.shaderInputDraws>0&&receipt.shaderInputDraws==receipt.shaderInputCopies)
                    appliedShaders=receipt.shaderInputs==null?null:receipt.shaderInputs.withUnchangedContext(draw.shaderInputs);
                else if(receipt.shaderInputDraws==0&&receipt.shaderInputCopies==0&&ActorShaderInputs.same(receipt.shaderInputs,draw.shaderInputs))
                    appliedShaders=draw.shaderInputs;
                else appliedShaders=null;
            }
            System.arraycopy(applied,0,rgba,role*4,4);
            dithering[role]=(byte)(flag==null?0:flag?2:1);
            if(appliedShaders!=null)shaders[role]=appliedShaders.copy();
        }
        return new LayerColorSample(cape|(wing<<2),identities,rgba,dithering,shaders);
    }
    int state(int role) {if(role<0||role>1)throw new IllegalArgumentException("invalid layer color role");return (states>>(role*2))&3;}
    float[] color(int role) {return state(role)==2?Arrays.copyOfRange(rgba,role*4,role*4+4):null;}
    private static byte[] identity(String geometry,String nativeCape) throws IOException {
        try {
            MessageDigest hash=MessageDigest.getInstance("SHA-256");hash.update(geometry.getBytes(StandardCharsets.UTF_8));hash.update((byte)0);
            if(nativeCape!=null)hash.update(nativeCape.getBytes(StandardCharsets.UTF_8));return hash.digest();
        }catch(NoSuchAlgorithmException missing){throw new IOException(missing);}
    }
    float[] colorFor(int role,String geometry,String nativeCape) throws IOException {
        if(state(role)!=2||geometry==null)return null;
        byte[] expected=identity(geometry,nativeCape);
        for(int i=0;i<32;i++)if(identities[role*32+i]!=expected[i])return null;return color(role);
    }
    Boolean ditheringFor(int role,String geometry,String nativeCape) throws IOException {
        return colorFor(role,geometry,nativeCape)==null||dithering[role]==0?null:dithering[role]==2;
    }
    ActorShaderInputs shaderInputsFor(int role,String geometry,String nativeCape) throws IOException {
        return colorFor(role,geometry,nativeCape)==null||shaders[role]==null?null:shaders[role].copy();
    }
    LayerColorSample copy(){return new LayerColorSample(states,identities,rgba,dithering,shaders);}
    boolean same(LayerColorSample other){return other!=null&&states==other.states&&Arrays.equals(identities,other.identities)
        &&Arrays.equals(rgba,other.rgba)&&Arrays.equals(dithering,other.dithering)
        &&ActorShaderInputs.same(shaders[0],other.shaders[0])&&ActorShaderInputs.same(shaders[1],other.shaders[1]);}
    private boolean sameIdentity(LayerColorSample other,int role) {
        for(int i=role*32;i<role*32+32;i++)if(identities[i]!=other.identities[i])return false;return true;
    }
    static LayerColorSample between(LayerColorSample a,LayerColorSample b,double phase) {
        if(!Double.isFinite(phase)||phase<0||phase>1)throw new IllegalArgumentException("invalid layer color phase");
        if(phase==0)return a.copy();if(phase==1)return b.copy();
        if(a.same(b))return a.copy();
        byte[] identities=new byte[64],dithering=new byte[2];float[] rgba=new float[8];int states=0;
        ActorShaderInputs[] shaders=new ActorShaderInputs[2];
        for(int role=0;role<2;role++) {
            int left=a.state(role),right=b.state(role),state=left==1&&right==1?1:0;
            if(left==2&&right==2&&a.sameIdentity(b,role)) {
                state=2;System.arraycopy(a.identities,role*32,identities,role*32,32);
                for(int i=role*4;i<role*4+4;i++)rgba[i]=Float.floatToIntBits(a.rgba[i])==Float.floatToIntBits(b.rgba[i])
                    ?a.rgba[i]:(float)(a.rgba[i]+((double)b.rgba[i]-a.rgba[i])*phase);
                if(a.dithering[role]==b.dithering[role])dithering[role]=a.dithering[role];
                shaders[role]=ActorShaderInputs.between(a.shaders[role],b.shaders[role]);
            }
            states|=state<<(role*2);
        }
        return new LayerColorSample(states,identities,rgba,dithering,shaders);
    }
    void write(DataOutput output) throws IOException {write(output,1);}
    void write(DataOutput output,int version) throws IOException {
        if(version<1||version>6)throw new IOException("unsupported layer color version");
        output.writeByte(states);
        for(int role=0;role<2;role++)if(state(role)==2) {
            output.write(identities,role*32,32);for(int i=role*4;i<role*4+4;i++)output.writeFloat(rgba[i]);
            if(version>=2)output.writeByte(dithering[role]);
            if(version>=3){output.writeBoolean(shaders[role]!=null);if(shaders[role]!=null)shaders[role].write(output,version>=4,version>=5,version>=6);}
        }
    }
    static LayerColorSample read(DataInput input) throws IOException {return read(input,1);}
    static LayerColorSample read(DataInput input,int version) throws IOException {
        if(version<1||version>6)throw new IOException("unsupported layer color version");
        int states=input.readUnsignedByte();byte[] identities=new byte[64],dithering=new byte[2];float[] rgba=new float[8];
        ActorShaderInputs[] shaders=new ActorShaderInputs[2];
        for(int role=0;role<2;role++)if(((states>>(role*2))&3)==2) {
            input.readFully(identities,role*32,32);for(int i=role*4;i<role*4+4;i++)rgba[i]=input.readFloat();
            if(version>=2)dithering[role]=(byte)input.readUnsignedByte();
            if(version>=3) {
                int present=input.readUnsignedByte();if(present>1)throw new IOException("invalid recorded shader-input presence");
                if(present==1)shaders[role]=ActorShaderInputs.read(input,version>=4,version>=5,version>=6);
            }
        }
        try{return new LayerColorSample(states,identities,rgba,dithering,shaders);}catch(IllegalArgumentException invalid){throw new IOException("invalid recorded layer color",invalid);}
    }
}
