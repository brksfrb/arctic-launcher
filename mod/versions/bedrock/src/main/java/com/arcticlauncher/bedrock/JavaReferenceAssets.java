package com.arcticlauncher.bedrock;

import com.google.gson.JsonObject;
import java.awt.image.BufferedImage;
import java.io.*;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.*;
import java.util.zip.*;

import com.arcticlauncher.client.Lang;
/** Asset-only access to an explicitly selected local Java client archive.
 * Never puts that archive on a class path, loads classes or extracts files.
 * The pinned resource is intentionally independent of Bedrock's pack stack. */
final class JavaReferenceAssets {
    private static final String GLINT="assets/minecraft/textures/misc/enchanted_glint_armor.png";
    static final String GLINT_SHA256="591205347810d37b6d6070a502468bbb1f2686fc0ab38b826b06cbba6349b23c";
    private static final String METADATA_SHA256="a4f5e3c3235dd128ee6d2ce397217e8c77037dbcf641aaa5bef65824e3b4b168";
    private static final long MAX_ARCHIVE=512L*1024*1024;
    private static final int MAX_ENTRIES=65536,MAX_PNG=65536,MAX_METADATA=4096;
    private final BufferedImage armorGlint;
    private final String stage,error;

    JavaReferenceAssets(File archive) {
        BufferedImage pixels=null;String state="unbound",failure=null;
        if(archive!=null) {
            try {pixels=load(archive);state="ready";}
            catch(IOException|RuntimeException invalid) {
                // Diagnostics deliberately omit the user's local archive path.
                state="rejected";failure="local Java reference asset could not be verified"; // i18n-ignore
            }
        }
        armorGlint=pixels;stage=state;error=failure;
    }

    BufferedImage armorGlint(){return armorGlint;}
    /** Bounded owned straight RGBA copy for the native glint uploader. The
     * selected archive and sampler metadata were verified during construction. */
    byte[] armorGlintRgba() {
        if(armorGlint==null)return null;
        byte[] result=new byte[128*128*4];int at=0;
        for(int y=0;y<128;y++)for(int x=0;x<128;x++) {
            int argb=armorGlint.getRGB(x,y);
            result[at++]=(byte)(argb>>>16);result[at++]=(byte)(argb>>>8);
            result[at++]=(byte)argb;result[at++]=(byte)(argb>>>24);
        }
        return result;
    }

    JsonObject status() {
        JsonObject state=new JsonObject();state.addProperty("stage",stage);state.addProperty("error",error);
        state.addProperty("referenceVersion","26.2");state.addProperty("armorGlintLoaded",armorGlint!=null);
        state.addProperty("armorGlintSha256",armorGlint==null?null:GLINT_SHA256);
        state.addProperty("sampling","linear-repeat");state.addProperty("assetOnly",true);
        state.addProperty("glintRenderingVerified",false);return state;
    }

    private static BufferedImage load(File archive) throws IOException {
        if(!archive.isAbsolute())throw new IOException("reference archive must be absolute");
        Path path=archive.toPath().toRealPath();
        BasicFileAttributes before=Files.readAttributes(path,BasicFileAttributes.class);
        if(!before.isRegularFile()||before.size()<1||before.size()>MAX_ARCHIVE)throw new IOException("invalid archive size");
        byte[] png,metadata;
        try(ZipFile zip=new ZipFile(path.toFile())) {
            ZipEntry image=null,meta=null;int count=0;
            Enumeration<? extends ZipEntry> entries=zip.entries();
            while(entries.hasMoreElements()) {
                ZipEntry entry=entries.nextElement();if(++count>MAX_ENTRIES)throw new IOException("archive entry limit");
                if(GLINT.equals(entry.getName())) {
                    if(image!=null)throw new IOException("duplicate reference texture");image=entry;
                } else if((GLINT+".mcmeta").equals(entry.getName())) {
                    if(meta!=null)throw new IOException("duplicate reference metadata");meta=entry;
                }
            }
            png=read(zip,image,MAX_PNG);metadata=read(zip,meta,MAX_METADATA);
        }
        BasicFileAttributes after=Files.readAttributes(path,BasicFileAttributes.class);
        if(!after.isRegularFile()||before.size()!=after.size()||!before.lastModifiedTime().equals(after.lastModifiedTime())
                ||!Objects.equals(before.fileKey(),after.fileKey()))throw new IOException("reference archive changed");
        if(!GLINT_SHA256.equals(sha256(png)))throw new IOException("reference texture differs from Java 26.2");
        // Pin the small metadata entry too: blur=true with clamp omitted.
        // This rejects altered sampler declarations and duplicate JSON keys.
        if(!METADATA_SHA256.equals(sha256(metadata)))throw new IOException("reference sampler metadata differs");
        BufferedImage result=AvatarRenderer.png(png);
        if(result.getWidth()!=128||result.getHeight()!=128)throw new IOException("invalid reference dimensions");
        return result;
    }

    private static byte[] read(ZipFile zip,ZipEntry entry,int limit) throws IOException {
        if(entry==null||entry.isDirectory()||entry.getSize()<1||entry.getSize()>limit)throw new IOException("invalid reference entry");
        int size=(int)entry.getSize();byte[] bytes=new byte[size];
        try(InputStream input=zip.getInputStream(entry)) {
            int at=0;
            while(at<size) {int n=input.read(bytes,at,size-at);if(n<1)throw new IOException("truncated reference entry");at+=n;}
            if(input.read()!=-1)throw new IOException("reference entry exceeds its declared size");
        }
        return bytes;
    }

    private static String sha256(byte[] bytes) {
        try {
            byte[] hash=MessageDigest.getInstance("SHA-256").digest(bytes);StringBuilder hex=new StringBuilder(64);
            for(byte value:hash)hex.append(Character.forDigit((value>>>4)&15,16)).append(Character.forDigit(value&15,16));
            return hex.toString();
        } catch(NoSuchAlgorithmException unavailable) {throw new IllegalStateException(unavailable);}
    }
}
