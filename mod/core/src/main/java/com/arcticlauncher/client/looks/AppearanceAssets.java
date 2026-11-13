package com.arcticlauncher.client.looks;

import com.arcticlauncher.client.Platform;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import java.util.zip.CRC32;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import java.util.zip.ZipFile;

/** Raw, content-addressed appearance files, before skin/cape preparation.
 * A snapshot copies checked bytes; cache filenames alone never prove content. */
public final class AppearanceAssets {
    private static final int MAX_FILE=4*1024*1024, MAX_FILES=4096;
    private static final long MAX_BYTES=256L*1024*1024;
    private final Path root;

    public AppearanceAssets(File directory) { root=directory.toPath().toAbsolutePath().normalize(); }

    public static byte[] fetch(Platform platform,String base,String kind,String hash) throws IOException {
        key(kind,hash);
        String path="texture".equals(kind)?"/v1/textures/"+hash+".png":"/v1/assets/"+hash;
        AppearanceAssets store=platform==null?null:platform.appearanceAssets();
        if(store==null)return Http.get(base+path,null);
        byte[] cached=null;
        try {cached=store.read(kind,hash);} catch(IOException unavailable) {platform.log(false,"Appearance cache: "+unavailable.getMessage());}
        if(cached!=null)return cached;
        byte[] bytes=Http.get(base+path,null);
        check(kind,hash,bytes);
        try {store.put(kind,hash,bytes);}
        catch(IOException unavailable) {platform.log(false,"Appearance cache: "+unavailable.getMessage());}
        return bytes;
    }

    /** A sorted archive key, with no caller-controlled path segments. */
    public static final class Ref implements Comparable<Ref> {
        public final String kind,hash;
        public Ref(String kind,String hash) { key(kind,hash);this.kind=kind;this.hash=hash; }
        String name() {return "assets/"+kind+"/"+hash;}
        @Override public int compareTo(Ref other) {return name().compareTo(other.name());}
    }

    private static String key(String kind,String hash) {
        if(!("asset".equals(kind)||"texture".equals(kind)) || hash==null || !hash.matches("[0-9a-f]{40}"))
            throw new IllegalArgumentException("invalid appearance asset reference");
        return kind+"-"+hash;
    }
    private static void check(String kind,String hash,byte[] bytes) throws IOException {
        key(kind,hash);
        if(bytes==null||bytes.length<1||bytes.length>MAX_FILE||!hash.equals(digest("SHA-1",bytes)))
            throw new IOException("appearance asset does not match its content hash: "+kind+"/"+hash);
    }
    static String digest(String algorithm,byte[] bytes) throws IOException {
        try {
            StringBuilder out=new StringBuilder();
            for(byte b:MessageDigest.getInstance(algorithm).digest(bytes))out.append(String.format(Locale.ROOT,"%02x",b&255));
            return out.toString();
        } catch(java.security.NoSuchAlgorithmException missing) {throw new IOException(missing);}
    }

    /** Retain adapter-owned bytes under their actual content hash. */
    public synchronized Ref put(String kind,byte[] bytes) throws IOException {
        if(bytes==null||bytes.length<1||bytes.length>MAX_FILE)throw new IOException("appearance asset exceeds its size limit");
        Ref ref=new Ref(kind,digest("SHA-1",bytes));put(ref.kind,ref.hash,bytes);return ref;
    }

    public synchronized byte[] read(String kind,String hash) throws IOException {
        Path file=root.resolve(key(kind,hash));
        if(!Files.isRegularFile(file,LinkOption.NOFOLLOW_LINKS))return null;
        try(InputStream input=Files.newInputStream(file)) {
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();byte[] buffer=new byte[16384];int n;
            while((n=input.read(buffer))!=-1) {
                if(bytes.size()+n>MAX_FILE)return null;
                bytes.write(buffer,0,n);
            }
            byte[] result=bytes.toByteArray();
            try {check(kind,hash,result);} catch(IOException invalid) {return null;}
            // Recency affects eviction only, never the exported archive.
            try {Files.setLastModifiedTime(file,java.nio.file.attribute.FileTime.fromMillis(System.currentTimeMillis()));}
            catch(IOException ignored) { }
            return result;
        }
    }

    public synchronized void put(String kind,String hash,byte[] bytes) throws IOException {
        check(kind,hash,bytes);
        Files.createDirectories(root);
        if(read(kind,hash)!=null)return;
        Path target=root.resolve(key(kind,hash)), temporary=Files.createTempFile(root,".appearance-",".tmp");
        try {
            Files.write(temporary,bytes);
            try {Files.move(temporary,target,StandardCopyOption.ATOMIC_MOVE,StandardCopyOption.REPLACE_EXISTING);}
            catch(AtomicMoveNotSupportedException unavailable) {Files.move(temporary,target,StandardCopyOption.REPLACE_EXISTING);}
        } finally {Files.deleteIfExists(temporary);}
        trim();
    }

    private void trim() throws IOException {
        List<CacheFile> files=new ArrayList<CacheFile>();long total=0;
        try(DirectoryStream<Path> entries=Files.newDirectoryStream(root)) {
            for(Path file:entries)if(file.getFileName().toString().matches("(?:asset|texture)-[0-9a-f]{40}")
                    && Files.isRegularFile(file,LinkOption.NOFOLLOW_LINKS)) {
                CacheFile entry=new CacheFile(file,Files.size(file),modified(file));files.add(entry);total+=entry.bytes;
            }
        }
        Collections.sort(files,(a,b)->Long.compare(a.modified,b.modified));
        int count=files.size();
        for(CacheFile file:files) {
            if(count<=MAX_FILES&&total<=MAX_BYTES)break;
            Files.delete(file.path);total-=file.bytes;count--;
        }
    }
    private static final class CacheFile {
        final Path path;final long bytes,modified;
        CacheFile(Path path,long bytes,long modified) {this.path=path;this.bytes=bytes;this.modified=modified;}
    }
    private static long modified(Path file) {
        try {return Files.getLastModifiedTime(file,LinkOption.NOFOLLOW_LINKS).toMillis();}
        catch(IOException unavailable) {return 0;}
    }

    /** Write a new portable snapshot. Entry order/times and manifest omit wall
     * time, paths and credentials; each asset includes an independent SHA-256. */
    public synchronized void archive(File output,JsonObject state,Collection<Ref> references) throws IOException {
        archive(output,state,references,(kind,hash)->read(kind,hash));
    }
    public interface Source { byte[] read(String kind,String hash) throws IOException; }
    public synchronized void archive(File output,JsonObject state,Collection<Ref> references,Source source) throws IOException {
        if(!output.isAbsolute()||!output.getName().endsWith(".arctic-looks.zip")||output.toPath().normalize().startsWith(root))
            throw new IOException("appearance snapshot needs a new absolute .arctic-looks.zip path outside its cache");
        SortedSet<Ref> sorted=new TreeSet<Ref>(references);
        if(sorted.size()>MAX_FILES)throw new IOException("appearance snapshot exceeds asset limit");
        JsonObject manifest=new com.google.gson.JsonParser().parse(state.toString()).getAsJsonObject();
        JsonArray assets=new JsonArray();long total=0;Map<String,String> hashes=new HashMap<String,String>();
        for(Ref ref:sorted) {
            byte[] bytes=source.read(ref.kind,ref.hash);
            if(bytes==null)throw new IOException("appearance snapshot is missing "+ref.kind+"/"+ref.hash);
            check(ref.kind,ref.hash,bytes);
            total+=bytes.length;if(total>MAX_BYTES)throw new IOException("appearance snapshot exceeds 256 MiB");
            JsonObject asset=new JsonObject();asset.addProperty("kind",ref.kind);asset.addProperty("hash",ref.hash);
            String sha256=digest("SHA-256",bytes);hashes.put(ref.name(),sha256);
            asset.addProperty("size",bytes.length);asset.addProperty("sha256",sha256);assets.add(asset);
        }
        manifest.add("assets",assets);
        byte[] json=manifest.toString().getBytes(StandardCharsets.UTF_8);
        if(json.length>1024*1024)throw new IOException("appearance manifest exceeds 1 MiB");
        Path target=output.toPath().normalize();boolean created=false,complete=false;
        try {
            OutputStream raw=Files.newOutputStream(target,StandardOpenOption.CREATE_NEW,StandardOpenOption.WRITE);created=true;
            try(ZipOutputStream zip=new ZipOutputStream(new BufferedOutputStream(raw))) {
                entry(zip,"manifest.json",json);
                for(Ref ref:sorted) {
                    byte[] bytes=source.read(ref.kind,ref.hash);
                    if(bytes==null||!digest("SHA-256",bytes).equals(hashes.get(ref.name())))throw new IOException("appearance asset changed while archiving");
                    entry(zip,ref.name(),bytes);
                }
            }
            complete=true;
        } finally {if(created&&!complete)Files.deleteIfExists(target);}
    }
    private static void entry(ZipOutputStream zip,String name,byte[] bytes) throws IOException {
        CRC32 crc=new CRC32();crc.update(bytes);
        ZipEntry entry=new ZipEntry(name);entry.setTime(0);entry.setMethod(ZipEntry.STORED);
        entry.setSize(bytes.length);entry.setCompressedSize(bytes.length);entry.setCrc(crc.getValue());
        zip.putNextEntry(entry);zip.write(bytes);zip.closeEntry();
    }

    /** Read without extracting or retaining every raw asset in memory. */
    public static final class Archive implements Closeable {
        private final ZipFile zip;
        private final JsonObject manifest;
        public final String manifestHash;
        private final SortedMap<String,Asset> assets=new TreeMap<String,Asset>();
        private static final class Asset {
            final Ref ref;final int size;final String sha256;
            Asset(Ref ref,int size,String sha256) {this.ref=ref;this.size=size;this.sha256=sha256;}
        }
        public Archive(File file) throws IOException {
            if(file==null||!file.isAbsolute()||!file.isFile()||file.length()>MAX_BYTES+2*1024*1024L)
                throw new IOException("invalid appearance archive path or size");
            zip=new ZipFile(file,StandardCharsets.UTF_8);
            try {
                Set<String> entries=new HashSet<String>();
                Enumeration<? extends ZipEntry> names=zip.entries();
                while(names.hasMoreElements()) {
                    ZipEntry entry=names.nextElement();String name=entry.getName();
                    if(entry.isDirectory()||entries.size()>=MAX_FILES+1||!entries.add(name)
                            || !("manifest.json".equals(name)||name.matches("assets/(?:texture|asset)/[0-9a-f]{40}")))
                        throw new IOException("appearance archive has duplicate, unknown or excessive entries");
                }
                ZipEntry header=zip.getEntry("manifest.json");
                if(header==null||header.getSize()<1||header.getSize()>1024*1024)throw new IOException("appearance archive has no bounded manifest");
                byte[] bytes=bounded(header,1024*1024);
                manifest=new com.google.gson.JsonParser().parse(new String(bytes,StandardCharsets.UTF_8)).getAsJsonObject();
                manifestHash=digest("SHA-256",bytes);
                if(integer(manifest,"format",2)<1||integer(manifest,"rig",2)<1)
                    throw new IOException("unsupported appearance snapshot format or rig");
                com.google.gson.JsonElement list=manifest.get("assets");
                if(list==null||!list.isJsonArray()||list.getAsJsonArray().size()>MAX_FILES)throw new IOException("invalid appearance asset index");
                long total=0;
                for(com.google.gson.JsonElement element:list.getAsJsonArray()) {
                    JsonObject object=element.getAsJsonObject();
                    Ref ref=new Ref(object.get("kind").getAsString(),object.get("hash").getAsString());
                    int size=(int)integer(object,"size",MAX_FILE);String sha256=object.get("sha256").getAsString();
                    ZipEntry entry=zip.getEntry(ref.name());
                    if(size<1||!sha256.matches("[0-9a-f]{64}")||entry==null||entry.getSize()!=size
                            ||assets.put(ref.name(),new Asset(ref,size,sha256))!=null|| (total+=size)>MAX_BYTES)
                        throw new IOException("invalid appearance asset descriptor");
                }
                if(entries.size()!=assets.size()+1)throw new IOException("appearance archive has unindexed assets");
            } catch(IOException failure) {zip.close();throw failure;}
            catch(RuntimeException|StackOverflowError invalid) {zip.close();throw new IOException("invalid appearance archive manifest",invalid);}
        }
        public JsonObject manifest() {return new com.google.gson.JsonParser().parse(manifest.toString()).getAsJsonObject();}
        public List<Ref> references() {
            List<Ref> result=new ArrayList<Ref>();for(Asset asset:assets.values())result.add(asset.ref);return result;
        }
        public byte[] read(String kind,String hash) throws IOException {
            return read(kind,hash,MAX_FILE);
        }
        /** A narrower adapter bound is checked before allocating entry bytes.
         * Both archive hashes and the manifest size are still verified. */
        public byte[] read(String kind,String hash,int maximum) throws IOException {
            if(maximum<1||maximum>MAX_FILE)throw new IOException("invalid appearance asset read bound");
            Ref ref=new Ref(kind,hash);Asset asset=assets.get(ref.name());
            if(asset==null)throw new IOException("appearance archive is missing "+kind+"/"+hash);
            if(asset.size>maximum)throw new IOException("appearance asset exceeds its adapter size limit");
            byte[] bytes=bounded(zip.getEntry(ref.name()),maximum);check(kind,hash,bytes);
            if(bytes.length!=asset.size||!digest("SHA-256",bytes).equals(asset.sha256))throw new IOException("appearance archive content changed or failed integrity checks");
            return bytes;
        }
        private byte[] bounded(ZipEntry entry,int maximum) throws IOException {
            try(InputStream input=zip.getInputStream(entry)) {
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();byte[] buffer=new byte[16384];int n;
                while((n=input.read(buffer))!=-1) {
                    if(bytes.size()+n>maximum)throw new IOException("appearance archive entry exceeds its limit");
                    bytes.write(buffer,0,n);
                }
                return bytes.toByteArray();
            }
        }
        private static long integer(JsonObject value,String field,long max) throws IOException {
            com.google.gson.JsonElement element=value.get(field);
            if(element==null||!element.isJsonPrimitive()||!element.getAsJsonPrimitive().isNumber())throw new IOException("invalid appearance archive integer");
            double number=element.getAsDouble();long integer=element.getAsLong();
            if(!Double.isFinite(number)||integer<0||integer>max||number!=integer)throw new IOException("appearance archive integer is out of range");
            return integer;
        }
        @Override public void close() throws IOException {zip.close();}
    }
}
