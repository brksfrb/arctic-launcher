package com.arcticlauncher.bedrock;

import com.google.gson.stream.JsonReader;
import com.google.gson.stream.JsonToken;
import java.io.*;
import java.util.*;

/** Reads one static atlas key without constructing every texture definition.
 * A missing key may fall through the pack stack; an unsupported definition may not. */
final class ItemAtlas {
    private ItemAtlas() {}
    static String resolve(byte[] bytes,String key,Integer frame) throws IOException {
        if(frame!=null&&(frame<0||frame>65535))throw new IOException("invalid observed item icon frame");
        if(bytes==null||bytes.length>8*1024*1024)throw new IOException("item atlas exceeds 8 MiB");
        if(bytes.length>=4&&bytes[0]==127&&bytes[1]=='M'&&bytes[2]=='C'&&bytes[3]=='B')
            throw new IOException("compiled item atlas needs a matching MCB schema");
        try(JsonReader input=new JsonReader(new StringReader(PackAssets.utf8(bytes,0,bytes.length)))) {
            // Bedrock's own reference catalogs include comments. Keep JSONC
            // compatibility while explicitly checking types and document end.
            input.setLenient(true);
            object(input);Set<String> fields=new HashSet<String>();String result=null;boolean data=false;
            while(input.hasNext()) {
                String field=name(input,fields,64,128);
                if("texture_name".equals(field)) {
                    if(!"atlas.items".equals(string(input,128)))throw new IOException("resource is not an item atlas");
                } else if("texture_data".equals(field)) {
                    data=true;object(input);Set<String> names=new HashSet<String>();
                    while(input.hasNext()) {
                        String icon=name(input,names,65536,193);
                        if(key.equals(icon))result=definition(input,frame);else input.skipValue();
                    }
                    input.endObject();
                } else input.skipValue();
            }
            input.endObject();
            if(!data||input.peek()!=JsonToken.END_DOCUMENT)throw new IOException("invalid item atlas document");
            return result;
        }
    }
    private static String definition(JsonReader input,Integer frame) throws IOException {
        object(input);Set<String> fields=new HashSet<String>();String texture=null;
        while(input.hasNext()) {
            String field=name(input,fields,16,128);
            if(!"textures".equals(field))throw new IOException("item icon definition requires an engine renderer: "+field);
            if(input.peek()==JsonToken.BEGIN_ARRAY) {
                input.beginArray();
                int count=0;
                while(input.hasNext()) {
                    if(count>=65536)throw new IOException("item icon has too many variants");
                    String path=string(input,512);
                    if(count==(frame==null?0:frame))texture=path;
                    count++;
                }
                input.endArray();
                if(count==0)throw new IOException("item icon has no texture");
                if(frame==null&&count!=1)throw new IOException("item icon needs an observed variant or engine renderer");
                if(frame!=null&&frame>=count)throw new IOException("observed item icon frame exceeds its texture variants");
            } else {
                texture=string(input,512);
                if(frame!=null&&frame!=0)throw new IOException("observed item icon frame does not select a single texture");
            }
        }
        input.endObject();
        if(texture==null)throw new IOException("item icon has no texture");
        if(texture.lastIndexOf('.')<=texture.lastIndexOf('/'))texture+=".png";
        PackAssets.path(texture);
        if(!texture.endsWith(".png"))throw new IOException("item icon is not a static PNG");
        return texture;
    }
    private static String name(JsonReader input,Set<String> seen,int count,int length) throws IOException {
        String name=input.nextName();
        if(name.isEmpty()||name.length()>length||seen.size()>=count||!seen.add(name))throw new IOException("duplicate, invalid or excessive item atlas keys");
        return name;
    }
    private static String string(JsonReader input,int max) throws IOException {
        if(input.peek()!=JsonToken.STRING)throw new IOException("item atlas field must be a string");
        String value=input.nextString();
        if(value.isEmpty()||value.length()>max)throw new IOException("invalid item atlas string length");
        return value;
    }
    private static void object(JsonReader input) throws IOException {
        if(input.peek()!=JsonToken.BEGIN_OBJECT)throw new IOException("item atlas field must be an object");
        input.beginObject();
    }
}
