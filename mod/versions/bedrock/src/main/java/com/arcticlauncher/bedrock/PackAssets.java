package com.arcticlauncher.bedrock;

import java.io.*;
import java.nio.*;
import java.nio.charset.*;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.*;

/** Read-only pack instances, highest priority first. Each instance has its own
 * ordered subpack/overlay/base roots. No discovery or priority inference. */
final class PackAssets {
    private static final long MAGIC=0x267052a0b125277dL,MAX_ARCHIVE=256L*1024*1024;
    private final List<List<Path>> roots=new ArrayList<List<Path>>();
    private PackAssets() {}
    PackAssets(List<File> packs) throws IOException {
        if(packs==null||packs.isEmpty()||packs.size()>64)throw new IOException("resource stack needs 1–64 pack roots");
        List<List<File>> groups=new ArrayList<List<File>>();
        for(File pack:packs)groups.add(Collections.singletonList(pack));
        prepare(groups);
        Set<Path> unique=new HashSet<Path>();for(List<Path> group:roots)if(!unique.add(group.get(0)))throw new IOException("duplicate pack root");
    }
    static PackAssets grouped(List<List<File>> packs) throws IOException {
        PackAssets assets=new PackAssets();assets.prepare(packs);return assets;
    }
    private void prepare(List<List<File>> packs) throws IOException {
        if(packs==null||packs.isEmpty()||packs.size()>64)throw new IOException("resource stack needs 1–64 pack instances");
        int count=0;
        for(List<File> pack:packs) {
            if(pack==null||pack.isEmpty())throw new IOException("resource pack instance has no roots");
            List<Path> ordered=new ArrayList<Path>();Set<Path> unique=new HashSet<Path>();
            for(File directory:pack) {
                if(++count>64||directory==null||!directory.isAbsolute())throw new IOException("resource stack needs at most 64 absolute roots");
                Path root=directory.toPath().toRealPath();
                if(!Files.isDirectory(root))throw new IOException("resource pack root is not a directory");
                if(unique.add(root))ordered.add(root);
            }
            roots.add(Collections.unmodifiableList(ordered));
        }
    }
    int size(){return roots.size();}
    static void path(String path) throws IOException {name(path,true);}
    byte[] read(String path,int limit) throws IOException {
        for(int i=0;i<roots.size();i++) {byte[] data=read(i,path,limit);if(data!=null)return data;}
        return null;
    }
    byte[] read(int pack,String path,int limit) throws IOException {
        name(path,true);
        if(limit<1||limit>16*1024*1024)throw new IOException("resource byte limit exceeds 16 MiB");
        // ResourcePack::getResource chooses the first existing resource inside
        // one instance before catalog keys merge across different instances.
        for(Path root:roots.get(pack)) {byte[] bytes=read(root,path,limit);if(bytes!=null)return bytes;}
        return null;
    }
    private static byte[] read(Path root,String path,int limit) throws IOException {
        int slash=path.lastIndexOf('/');
        String directory=slash<0?"":path.substring(0,slash),file=path.substring(slash+1);
        // Each archive describes files immediately inside one directory.
        Path archive=directory.isEmpty()?null:existing(root,"__brarchive/"+directory+".brarchive");
        Entry entry=archive==null?null:archive(archive,file,limit);
        Path loose=existing(root,path);
        byte[] bytes=loose==null?null:loose(loose,limit);
        if(entry==null)return bytes;
        if(entry.stub)return bytes==null?new byte[0]:bytes;
        if(bytes!=null&&!Arrays.equals(bytes,entry.bytes))throw new IOException("resource has conflicting loose and archived payloads: "+path);
        return entry.bytes;
    }
    private static Path existing(Path root,String path) throws IOException {
        name(path,true);Path candidate=root.resolve(path);
        try {
            Path real=candidate.toRealPath();
            if(!real.startsWith(root)||!Files.isRegularFile(real))throw new IOException("resource escapes pack or is not a file");
            return real;
        } catch(NoSuchFileException missing) {return null;}
    }
    private static void name(String path,boolean directories) throws IOException {
        if(path==null||path.isEmpty()||path.length()>512||path.startsWith("/")||path.indexOf('\\')>=0||path.indexOf(':')>=0)
            throw new IOException("invalid pack resource path");
        for(int i=0;i<path.length();i++)if(Character.isISOControl(path.charAt(i)))throw new IOException("control character in resource path");
        for(String part:path.split("/",-1))if(part.isEmpty()||part.equals(".")||part.equals(".."))throw new IOException("invalid pack resource component");
        if(!directories&&path.indexOf('/')>=0)throw new IOException("archive entry must be a direct filename");
    }
    private static byte[] loose(Path path,int limit) throws IOException {
        BasicFileAttributes before=Files.readAttributes(path,BasicFileAttributes.class);
        if(before.size()>limit)throw new IOException("resource exceeds its byte limit");
        byte[] data=new byte[(int)before.size()];
        try(InputStream input=Files.newInputStream(path)) {
            int at=0;while(at<data.length) {int n=input.read(data,at,data.length-at);if(n<0)throw new EOFException("resource truncated");at+=n;}
            if(input.read()!=-1)throw new IOException("resource grew while reading");
        }
        unchanged(path,before);return data;
    }
    private static final class Entry {
        final byte[] bytes;final boolean stub;
        Entry(byte[] bytes,boolean stub){this.bytes=bytes;this.stub=stub;}
    }
    /** Uncompressed brarchive v1; entries may contain PNG/MCB bytes. No
     * extraction, decompressor, native call or UTF-8 assumption about payloads. */
    private static Entry archive(Path path,String wanted,int limit) throws IOException {
        BasicFileAttributes before=Files.readAttributes(path,BasicFileAttributes.class);
        if(before.size()<16||before.size()>MAX_ARCHIVE)throw new IOException("invalid resource archive size");
        try(RandomAccessFile file=new RandomAccessFile(path.toFile(),"r")) {
            byte[] header=new byte[16];file.readFully(header);
            ByteBuffer h=ByteBuffer.wrap(header).order(ByteOrder.LITTLE_ENDIAN);
            long magic=h.getLong(),count=Integer.toUnsignedLong(h.getInt()),version=Integer.toUnsignedLong(h.getInt());
            if(magic!=MAGIC||version!=1||count>65536)throw new IOException("unsupported resource archive header");
            long base=16+count*256;
            if(base>before.size())throw new IOException("truncated resource archive index");
            long offset=-1,length=0,descriptor=-1;byte[] selected=null;
            byte[] entry=new byte[256];
            for(long i=0;i<count;i++) {
                file.readFully(entry);int size=entry[0]&255;
                if(size<1||size>247)throw new IOException("invalid resource archive filename length");
                for(int j=1+size;j<248;j++)if(entry[j]!=0)throw new IOException("invalid resource archive name padding");
                String item=utf8(entry,1,size);name(item,false);
                ByteBuffer b=ByteBuffer.wrap(entry).order(ByteOrder.LITTLE_ENDIAN);
                long start=Integer.toUnsignedLong(b.getInt(248)),bytes=Integer.toUnsignedLong(b.getInt(252));
                if(start>before.size()-base||bytes>before.size()-base-start)throw new IOException("resource archive entry exceeds its content section");
                if(item.equals(wanted)) {
                    if(offset>=0)throw new IOException("ambiguous duplicate resource archive entry");
                    if(bytes>limit)throw new IOException("archived resource exceeds its byte limit");
                    offset=start;length=bytes;descriptor=16+i*256;selected=entry.clone();
                }
            }
            Entry result=null;
            if(offset>=0) {
                byte[] data=new byte[(int)length];file.seek(base+offset);file.readFully(data);
                file.seek(descriptor);file.readFully(entry);
                if(!Arrays.equals(entry,selected))throw new IOException("resource archive index changed during read");
                result=new Entry(data,offset==0&&length==0);
            }
            file.seek(0);byte[] after=new byte[16];file.readFully(after);
            if(!Arrays.equals(header,after)||file.length()!=before.size())throw new IOException("resource archive changed during read");
            unchanged(path,before);return result;
        }
    }
    static String utf8(byte[] bytes,int start,int length) throws IOException {
        return StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes,start,length)).toString();
    }
    private static void unchanged(Path path,BasicFileAttributes before) throws IOException {
        BasicFileAttributes after=Files.readAttributes(path,BasicFileAttributes.class);
        if(before.size()!=after.size()||!before.lastModifiedTime().equals(after.lastModifiedTime())
                ||!Objects.equals(before.fileKey(),after.fileKey()))throw new IOException("resource source changed during read");
    }
}
