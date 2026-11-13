package com.arcticlauncher.bedrock;

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Owned final current-palette values in native root/pixel coordinates. A
 * completed observed collection is distinct from complete equipment coverage. */
final class AttachmentSample {
    static final int CAPE_SLOT=7,WING_SLOT=8;
    int schema;long animationFrame;boolean complete,coverageVerified;Palette[] palettes;
    static final class Palette {
        int slot,group,owner;long hierarchy;String geometry,definition;Bone[] bones;
        String nativeCape;ReplayReceipt replay;
        String structuralKey(){return slot+":"+hierarchy+":"+group+":"+owner+":"+definition+":"+geometry;}
        String key(){return structuralKey()+":"+nativeCape;}
    }
    /** Provenance from a successful native current-output copy. The archive
     * drops it; playback must obtain a new receipt at the requested clock. */
    static final class ReplayReceipt {
        String manifestHash;long timeMs;int timeRemainder,timeDenominator;
        void validate() {
            if(manifestHash==null||!manifestHash.matches("[0-9a-f]{64}")||timeMs<0||timeMs>86_400_000
                    ||timeDenominator<1||timeDenominator>1000||timeRemainder<0||timeRemainder>=timeDenominator
                    ||timeMs==86_400_000&&timeRemainder!=0)throw new IllegalArgumentException("invalid native palette replay receipt");
        }
        boolean matches(String manifest,CosmeticClock.Sample time) {
            return time!=null&&Objects.equals(manifestHash,manifest)&&timeMs==time.timeMs
                &&timeRemainder==time.remainder&&timeDenominator==time.denominator;
        }
    }
    static final class Bone {
        String name;int index,palette;Integer rigPart,anchorBone,ancestorDepth;float[] matrix;
    }
    void validate() {
        if(schema<1||schema>3||animationFrame<0||coverageVerified||palettes==null||palettes.length>16||!complete&&palettes.length!=0)
            throw new IllegalArgumentException("invalid observed attachment collection");
        Set<String> keys=new HashSet<String>();int count=0;
        for(Palette p:palettes) {
            if(p==null||p.slot<0||p.slot>WING_SLOT||p.group<0||p.group>=64||p.owner<0||p.owner>=64
                    ||p.slot==WING_SLOT&&!"geometry.elytra".equals(p.geometry)||schema<3&&p.slot==WING_SLOT
                    ||p.hierarchy<0||p.hierarchy>0xffff_ffffL||!text(p.geometry,128)||p.definition==null
                    ||!p.definition.matches("[0-9a-f]{64}")||p.bones==null||p.bones.length<1||p.bones.length>8||!keys.add(p.structuralKey())
                    ||(p.slot==CAPE_SLOT)!=(p.nativeCape!=null)||p.nativeCape!=null&&!p.nativeCape.matches("engine:cape/[0-9a-f]{64}")
                    ||schema==1&&(p.slot==CAPE_SLOT||p.replay!=null))
                throw new IllegalArgumentException("invalid observed attachment palette");
            if(p.replay!=null)p.replay.validate();
            count+=p.bones.length;if(count>128)throw new IllegalArgumentException("observed attachments exceed bone budget");
            for(int i=0;i<p.bones.length;i++) {
                Bone b=p.bones[i];
                if(b==null||!text(b.name,64)||b.index<0||b.index>=256||b.palette!=i
                        ||(b.rigPart==null)!=(b.anchorBone==null)||(b.rigPart==null)!=(b.ancestorDepth==null)
                        ||b.rigPart!=null&&(b.rigPart<0||b.rigPart>=6||b.anchorBone<0||b.anchorBone>=256||b.ancestorDepth<0||b.ancestorDepth>=256))
                    throw new IllegalArgumentException("invalid observed attachment bone");
                matrix(b.matrix);
            }
        }
    }
    void validate(WorldFrame.Player player) {
        validate();
        if(!"native-model-draw".equals(player.modelSource)||!Integer.valueOf(63).equals(player.availableParts)
                ||player.nativeGlobal==null||!player.nativeGlobal.has("animationFrame")
                ||player.nativeGlobal.get("animationFrame").getAsLong()!=animationFrame)
            throw new IllegalArgumentException("attachment collection belongs to another native draw");
        for(Palette p:palettes)if(p.slot==CAPE_SLOT&&(!Objects.equals(p.nativeCape,player.nativeCape)
                ||player.nativeCapeAbsent||player.nativeCapeSuppressed||player.nativeCapeHidden))
            throw new IllegalArgumentException("native cape palette belongs to another appearance");
        for(Palette p:palettes)if(p.slot==WING_SLOT&&(player.wingMatrices==null||!Boolean.TRUE.equals(player.elytraEquipped)||player.replaceElytra))
            throw new IllegalArgumentException("native wing palette needs its unsuppressed equipped layer");
    }
    private static boolean text(String value,int bytes) {
        if(value==null||value.isEmpty()||value.getBytes(StandardCharsets.UTF_8).length>bytes)return false;
        for(int i=0;i<value.length();i++)if(Character.isISOControl(value.charAt(i)))return false;return true;
    }
    private static void matrix(float[] matrix) {
        if(matrix==null||matrix.length!=16)throw new IllegalArgumentException("missing attachment matrix");
        for(float value:matrix)if(!Float.isFinite(value)||Math.abs(value)>1_000_000)throw new IllegalArgumentException("invalid attachment matrix");
        if(Math.abs(matrix[3])>.000001f||Math.abs(matrix[7])>.000001f||Math.abs(matrix[11])>.000001f||Math.abs(matrix[15]-1)>.000001f)
            throw new IllegalArgumentException("attachment matrix is not affine");
    }
    AttachmentSample copy() {
        validate();AttachmentSample out=new AttachmentSample();out.schema=schema;out.animationFrame=animationFrame;out.complete=complete;
        out.palettes=new Palette[palettes.length];
        for(int i=0;i<palettes.length;i++) {
            Palette p=palettes[i],q=new Palette();q.slot=p.slot;q.hierarchy=p.hierarchy;q.group=p.group;q.owner=p.owner;
            q.geometry=p.geometry;q.definition=p.definition;q.nativeCape=p.nativeCape;q.bones=new Bone[p.bones.length];
            for(int j=0;j<p.bones.length;j++) {
                Bone b=p.bones[j],c=new Bone();c.name=b.name;c.index=b.index;c.palette=b.palette;c.rigPart=b.rigPart;
                c.anchorBone=b.anchorBone;c.ancestorDepth=b.ancestorDepth;c.matrix=b.matrix.clone();q.bones[j]=c;
            }
            out.palettes[i]=q;
        }
        Arrays.sort(out.palettes,(a,b)->a.key().compareTo(b.key()));return out;
    }
    boolean sameLayout(AttachmentSample other) {
        if(other==null||complete!=other.complete||palettes.length!=other.palettes.length)return false;
        for(int i=0;i<palettes.length;i++) {
            Palette a=palettes[i],b=other.palettes[i];if(!a.key().equals(b.key())||a.bones.length!=b.bones.length)return false;
            for(int j=0;j<a.bones.length;j++) {
                Bone x=a.bones[j],y=b.bones[j];
                if(!x.name.equals(y.name)||x.index!=y.index||x.palette!=y.palette||!Objects.equals(x.rigPart,y.rigPart)
                        ||!Objects.equals(x.anchorBone,y.anchorBone)||!Objects.equals(x.ancestorDepth,y.ancestorDepth))return false;
            }
        }
        return true;
    }
    boolean same(AttachmentSample other) {
        if(!sameLayout(other))return false;
        // Native animation-frame counters can advance at an identical held
        // clock without changing any observed pose. They are provenance only.
        for(int i=0;i<palettes.length;i++)for(int j=0;j<palettes[i].bones.length;j++)
            if(!Arrays.equals(palettes[i].bones[j].matrix,other.palettes[i].bones[j].matrix))return false;return true;
    }
    static AttachmentSample between(AttachmentSample a,AttachmentSample b,double phase) {
        if(!Double.isFinite(phase)||phase<0||phase>1||!a.sameLayout(b))throw new IllegalArgumentException("attachment layout changed between samples");
        if(phase==1)return b.copy();AttachmentSample out=a.copy();if(phase==0)return out;
        for(int i=0;i<out.palettes.length;i++)for(int j=0;j<out.palettes[i].bones.length;j++)
            out.palettes[i].bones[j].matrix=AttachmentTransform.between(a.palettes[i].bones[j].matrix,b.palettes[i].bones[j].matrix,phase);
        out.validate();return out;
    }
    boolean hasNativeCape(String cape) {
        if(schema<2||cape==null||!complete)return false;
        for(Palette p:palettes)if(p.slot==CAPE_SLOT&&cape.equals(p.nativeCape))return true;return false;
    }
    AttachmentSample forNativeCape(String cape) {
        AttachmentSample out=copy();java.util.List<Palette> keep=new ArrayList<Palette>();
        for(Palette p:out.palettes)if(p.slot!=CAPE_SLOT||Objects.equals(cape,p.nativeCape))keep.add(p);
        out.palettes=keep.toArray(new Palette[keep.size()]);return out;
    }
    AttachmentSample forNativeWings(boolean enabled) {
        AttachmentSample out=copy();if(enabled)return out;
        java.util.List<Palette> keep=new ArrayList<Palette>();
        for(Palette p:out.palettes)if(p.slot!=WING_SLOT)keep.add(p);
        out.palettes=keep.toArray(new Palette[keep.size()]);return out;
    }
    boolean nativeCapeReplayed(AttachmentSample actual,String cape,String manifest,CosmeticClock.Sample time) {
        if(!hasNativeCape(cape)||actual==null||!actual.complete||actual.schema<2)return false;
        int expectedCount=0,actualCount=0;
        for(Palette p:palettes)if(p.slot==CAPE_SLOT) {
            if(!cape.equals(p.nativeCape))return false;expectedCount++;Palette match=null;
            for(Palette candidate:actual.palettes)if(p.key().equals(candidate.key())){match=candidate;break;}
            if(match==null||match.replay==null||!match.replay.matches(manifest,time)||match.bones.length!=p.bones.length)return false;
            for(int i=0;i<p.bones.length;i++)if(!p.bones[i].name.equals(match.bones[i].name)||p.bones[i].index!=match.bones[i].index
                    ||p.bones[i].palette!=match.bones[i].palette)return false;
        }
        for(Palette p:actual.palettes)if(p.slot==CAPE_SLOT)actualCount++;
        return expectedCount>0&&expectedCount==actualCount;
    }
    void write(DataOutput out) throws IOException {write(out,1);}
    void write(DataOutput out,int version) throws IOException {
        if(version<1||version>3)throw new IOException("unsupported attachment sample version");
        validate();out.writeLong(animationFrame);out.writeBoolean(complete);out.writeByte(palettes.length);
        for(Palette p:palettes) {
            if(version==1&&p.slot==CAPE_SLOT)throw new IOException("native cape needs attachment samples v2");
            if(version<3&&p.slot==WING_SLOT)throw new IOException("native wings need attachment samples v3");
            out.writeByte(p.slot);out.writeLong(p.hierarchy);out.writeByte(p.group);out.writeByte(p.owner);
            if(version>=2){out.writeBoolean(p.nativeCape!=null);if(p.nativeCape!=null)out.writeUTF(p.nativeCape);}
            out.writeUTF(p.geometry);out.writeUTF(p.definition);out.writeByte(p.bones.length);
            for(Bone b:p.bones) {
                out.writeUTF(b.name);out.writeShort(b.index);out.writeByte(b.palette);out.writeByte(b.rigPart==null?255:b.rigPart);
                if(b.rigPart!=null){out.writeShort(b.anchorBone);out.writeShort(b.ancestorDepth);}
                for(float value:b.matrix)out.writeFloat(value);
            }
        }
    }
    static AttachmentSample read(DataInput in) throws IOException {return read(in,1);}
    static AttachmentSample read(DataInput in,int version) throws IOException {
        if(version<1||version>3)throw new IOException("unsupported attachment sample version");
        AttachmentSample sample=new AttachmentSample();sample.schema=version;sample.animationFrame=in.readLong();int completed=in.readUnsignedByte();
        if(completed>1)throw new IOException("invalid attachment completion flag");sample.complete=completed==1;
        int count=in.readUnsignedByte();if(count>16)throw new IOException("attachment palette budget exceeded");sample.palettes=new Palette[count];int bones=0;
        for(int i=0;i<count;i++) {
            Palette p=new Palette();p.slot=in.readUnsignedByte();p.hierarchy=in.readLong();p.group=in.readUnsignedByte();p.owner=in.readUnsignedByte();
            if(version>=2){int cape=in.readUnsignedByte();if(cape>1)throw new IOException("invalid native cape identity flag");if(cape==1)p.nativeCape=in.readUTF();}
            p.geometry=in.readUTF();p.definition=in.readUTF();int size=in.readUnsignedByte();bones+=size;
            if(size<1||size>8||bones>128)throw new IOException("attachment bone budget exceeded");p.bones=new Bone[size];sample.palettes[i]=p;
            for(int j=0;j<size;j++) {
                Bone b=new Bone();b.name=in.readUTF();b.index=in.readUnsignedShort();b.palette=in.readUnsignedByte();int role=in.readUnsignedByte();
                if(role!=255){b.rigPart=role;b.anchorBone=in.readUnsignedShort();b.ancestorDepth=in.readUnsignedShort();}
                b.matrix=new float[16];for(int k=0;k<16;k++)b.matrix[k]=in.readFloat();p.bones[j]=b;
            }
        }
        try {return sample.copy();}catch(IllegalArgumentException invalid){throw new IOException("invalid recorded attachment palette",invalid);}
    }
}
