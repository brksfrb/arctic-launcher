package com.arcticlauncher.bedrock;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;

/** Versioned little-endian CPU staging contract, independent of engine ABI. */
final class NativeGeometryPacket {
    static final int MAX_BYTES=96*1024*1024;
    private NativeGeometryPacket() {}
    static byte[] encode(WorldFrame world,float[] equipment,NativeAvatarGeometry geometry) {
        return encode(world,equipment,geometry,null);
    }
    static byte[] encode(WorldFrame world,float[] equipment,NativeAvatarGeometry geometry,JavaEntityShader material) {
        return encode(world,equipment,geometry,material,null);
    }
    static byte[] encode(WorldFrame world,float[] equipment,NativeAvatarGeometry geometry,JavaEntityShader material,JavaSimplePhases.Result phases) {
        if(phases!=null)phases.requireSource(world,geometry);
        byte[] session=world.session.getBytes(StandardCharsets.US_ASCII);
        byte[] dimension=world.dimension.getBytes(StandardCharsets.UTF_8);
        double time=world.cosmeticTimeExactMillis();
        CosmeticClock.Sample replay=world.replayCosmeticSample();
        if(!world.session.matches("[A-Za-z0-9_-]{1,64}")||world.sequence<0||world.clockMs<0
                ||!Double.isFinite(time)||time<0||time>(double)Long.MAX_VALUE||dimension.length>256
                ||world.width<1||world.width>3840||world.height<1||world.height>2160
                ||world.camera==null||world.camera.length!=6||world.viewProjection==null||world.viewProjection.length!=16
                ||equipment!=null&&equipment.length!=16||material!=null&&world.cameraRelative==null)
            throw new IllegalArgumentException("invalid geometry world binding");
        for(double value:world.camera)if(!Double.isFinite(value)||Math.abs(value)>32_000_000)
            throw new IllegalArgumentException("invalid geometry camera");
        for(float value:world.viewProjection)if(!Float.isFinite(value))throw new IllegalArgumentException("invalid geometry projection");
        if(equipment!=null)for(float value:equipment)if(!Float.isFinite(value))throw new IllegalArgumentException("invalid equipment projection");
        boolean skinMaterial=false,skinVisibility=false,textureIdentity=false,layerVisibility=false,renderReferences=false,models=false,glint=false,glintActive=false;
        for(NativeAvatarGeometry.Batch batch:geometry.batches()) {
            if(batch.pass==NativeAvatarGeometry.Pass.SKIN_TRANSLUCENT)skinMaterial=true;
            if(batch.skinVisibilityKnown||batch.pass==NativeAvatarGeometry.Pass.BENT_SKIN)skinVisibility=true;
            if(batch.logicalTextureId!=null)textureIdentity=true;
            if(batch.layerVisibilityKnown)layerVisibility=true;
            if(batch.renderType!=null)renderReferences=true;
            if(batch.modelSubmission!=null)models=true;
            if(batch.modelSubmission!=null&&batch.modelSubmission.glint!=null) {
                glint=true;if(batch.modelSubmission.glint.selected)glintActive=true;
            }
        }
        int version=glint?10:phases!=null?9:models?8:renderReferences?7:layerVisibility?6:textureIdentity?5:skinVisibility?4:skinMaterial?3:material==null?1:2;
        if(glintActive&&(world.cameraRelative==null||equipment==null))throw new IllegalArgumentException("active glint metadata needs a declared layered camera");
        if((skinMaterial||skinVisibility)&&world.cameraRelative==null)throw new IllegalArgumentException("skin material needs declared geometry space");
        // Fixed header plus two length-prefixed strings and optional matrix.
        long size=8+4+session.length+8+8+8+1+(replay==null?0:16)+4+4+1+4+dimension.length+24+64+1+(equipment==null?0:64)
            +(version>=2?1+(material==null?0:4L*JavaEntityShader.WIRE_FLOATS):0)+4+36L*geometry.actors().size()+4+4;
        for(NativeAvatarGeometry.Batch batch:geometry.batches()) {
            if(batch.actor<0||batch.actor>=geometry.actors().size())throw new IllegalArgumentException("unbound geometry actor");
            size+=32+46L*batch.vertexCount+(batch.pass==NativeAvatarGeometry.Pass.SKIN_TRANSLUCENT?16:0);
            if(version>=5)size+=1+(batch.logicalTextureId==null?0:4+batch.logicalTextureId.length());
            if(version>=7)size+=1+(batch.renderType==null?0:9);
            if(version>=8)size+=1+(batch.modelSubmission==null?0:14);
            if(version>=10)size+=1+(batch.modelSubmission!=null&&batch.modelSubmission.glint!=null&&batch.modelSubmission.glint.selected?4L*GlintSample.FLOATS:0);
        }
        for(NativeAvatarGeometry.Texture texture:geometry.textures())size+=8+4L*texture.width*texture.height;
        if(version>=10)size+=1; // explicit known/unknown simple-phase trailer
        if(phases!=null) {
            size+=9;
            for(JavaSimplePhases.Phase phase:phases.phases)size+=6+4L*phase.batches().length;
        }
        if(size>MAX_BYTES)throw new IllegalArgumentException("native geometry packet exceeds 96 MiB");
        ByteBuffer out=ByteBuffer.allocate((int)size).order(ByteOrder.LITTLE_ENDIAN);
        out.put(new byte[]{'A','R','C','G','E','O',(byte)('0'+version/10),(byte)('0'+version%10)});
        string(out,session);out.putLong(world.sequence).putLong(world.clockMs).putDouble(time);
        out.put((byte)(replay==null?0:1));
        if(replay!=null)out.putLong(replay.timeMs).putInt(replay.remainder).putInt(replay.denominator);
        out.putInt(world.width).putInt(world.height);
        out.put((byte)(world.cameraRelative==null?0:world.cameraRelative?2:1));string(out,dimension);
        // WorldFrame carries x/y/z/yaw/pitch/FOV. Geometry stores only its
        // three double position coordinates; the matrix carries orientation.
        for(int axis=0;axis<3;axis++)out.putDouble(world.camera[axis]);
        for(float value:world.viewProjection)out.putFloat(value);
        out.put((byte)(equipment==null?0:1));if(equipment!=null)for(float value:equipment)out.putFloat(value);
        if(version>=2){out.put((byte)(material==null?0:1));if(material!=null)material.write(out);}
        out.putInt(geometry.actors().size());
        for(String actor:geometry.actors())out.put(actor.getBytes(StandardCharsets.US_ASCII));
        out.putInt(geometry.batches().size()).putInt(geometry.textures().size());
        for(NativeAvatarGeometry.Batch batch:geometry.batches()) {
            out.putShort((short)batch.actor).put(pass(batch.pass));
            // Bit 2 distinguishes an explicit scene-light input from the
            // software preview's full-bright fallback. Older readers reject it.
            out.put((byte)((batch.translucent?1:0)|(batch.emissive?2:0)|(batch.sceneLightKnown?4:0)|(batch.skinVisibilityKnown?8:0)
                |(batch.layerVisibilityKnown?16:0))).putInt(batch.texture);
            for(float value:batch.tint())out.putFloat(value);
            out.putInt(batch.vertexCount).putInt(batch.vertexCount/4*6);
            if(batch.pass==NativeAvatarGeometry.Pass.SKIN_TRANSLUCENT)for(float value:batch.skinOverlay())out.putFloat(value);
            if(version>=5) {
                out.put((byte)(batch.logicalTextureId==null?0:1));
                if(batch.logicalTextureId!=null)string(out,batch.logicalTextureId.getBytes(StandardCharsets.US_ASCII));
            }
            if(version>=7) {
                out.put((byte)(batch.renderType==null?0:1));
                if(batch.renderType!=null)out.put((byte)batch.renderType.factory.wire).putLong(batch.renderType.reference);
            }
            if(version>=8) {
                NativeAvatarGeometry.ModelSubmission model=batch.modelSubmission;
                out.put((byte)(model==null?0:1));
                if(model!=null)out.putShort((short)model.id).putFloat(model.x).putFloat(model.y).putFloat(model.z);
            }
            if(version>=10) {
                GlintSample sample=batch.modelSubmission==null?null:batch.modelSubmission.glint;
                if(sample==null)out.put((byte)0);else sample.write(out);
            }
            float[] vertices=batch.vertices();int[] colors=batch.colors(),lights=batch.lights();
            for(int i=0;i<batch.vertexCount;i++) {
                for(int component=0;component<8;component++)out.putFloat(vertices[i*8+component]);
                out.putInt(colors[i]).putInt(lights[i]);
            }
            for(int index:batch.indices())out.putInt(index);
        }
        for(NativeAvatarGeometry.Texture texture:geometry.textures()) {
            out.putInt(texture.width).putInt(texture.height);
            for(int pixel:texture.argb())out.putInt(pixel);
        }
        if(version>=10)out.put((byte)(phases==null?0:1));
        if(phases!=null) {
            out.putLong(phases.epoch).put((byte)phases.phases.size());
            for(JavaSimplePhases.Phase phase:phases.phases) {
                int[] batches=phase.batches();
                out.put((byte)phase.order).put((byte)phase.stage).putInt(batches.length);
                for(int batch:batches)out.putInt(batch);
            }
        }
        if(out.hasRemaining())throw new IllegalStateException("native geometry packet length disagrees with its data");
        return out.array();
    }
    private static void string(ByteBuffer out,byte[] value){out.putInt(value.length).put(value);}
    private static byte pass(NativeAvatarGeometry.Pass pass) {
        switch(pass) {
            case GENERAL:return 1;
            case COSMETIC_BASE:return 2;
            case COSMETIC_GLOW:return 3;
            case CAPE_SOLID:return 4;
            case WING_CUTOUT:return 5;
            case SKIN_TRANSLUCENT:return 6;
            case BENT_SKIN:return 7;
            default:throw new IllegalArgumentException("unsupported native geometry pass");
        }
    }
}
