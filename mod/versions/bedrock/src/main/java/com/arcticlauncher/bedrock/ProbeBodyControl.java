package com.arcticlauncher.bedrock;

import com.google.gson.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;

import com.arcticlauncher.client.Lang;
/** Fixed diagnostic modes for an explicitly enabled isolated body probe. */
final class ProbeBodyControl {
    static final float DEFAULT_ALPHA=0.0001f;
    static final class State {
        final String mode,method;final float alpha;
        State(String mode,float alpha,String method){this.mode=mode;this.alpha=alpha;this.method=method;}
        boolean allowReadiness(){return !"native".equals(mode);}
        boolean renderLayer(){return !"suppression".equals(mode);}
    }
    private final Path directory;
    private final boolean enabled;
    private long lastSequence;
    private State current=new State("replacement",DEFAULT_ALPHA,"bones");
    ProbeBodyControl(Path directory,boolean enabled){this.directory=directory;this.enabled=enabled;}
    State poll() {
        if(!enabled || directory==null)return current;
        Path request=directory.resolve("probe-body-control.json");
        if(!Files.exists(request))return current;
        long sequence=0;
        try {
            if(Files.size(request)>4096)throw new IllegalArgumentException("body control exceeds limit");
            JsonObject input=new JsonParser().parse(new String(Files.readAllBytes(request),StandardCharsets.UTF_8)).getAsJsonObject();
            if(input.get("schema").getAsInt()!=1)throw new IllegalArgumentException("invalid body control schema");
            sequence=input.get("sequence").getAsLong();
            if(sequence<=0 || sequence>1_000_000_000)throw new IllegalArgumentException("invalid body control sequence");
            if(sequence<=lastSequence)return current;
            String mode=input.get("mode").getAsString();
            if(!"native".equals(mode)&&!"replacement".equals(mode)&&!"suppression".equals(mode))
                throw new IllegalArgumentException("invalid body control mode");
            float alpha=input.has("alpha")?input.get("alpha").getAsFloat():DEFAULT_ALPHA;
            if(!Float.isFinite(alpha)||alpha<0||alpha>0.01f)throw new IllegalArgumentException("invalid body control alpha");
            String method=input.has("method")?input.get("method").getAsString():"bones";
            if(!"bones".equals(method)&&!"alpha".equals(method))throw new IllegalArgumentException("invalid body control method");
            State next=new State(mode,alpha,method);
            acknowledge(sequence,next,null);
            lastSequence=sequence;current=next;
        }catch(Exception error) {
            current=new State("native",DEFAULT_ALPHA,"bones");
            try{acknowledge(sequence,current,"body control rejected: "+error.getClass().getSimpleName()); // i18n-ignore
                if(sequence>0&&sequence<=1_000_000_000)lastSequence=Math.max(lastSequence,sequence);
            }catch(Exception ignored){}
        }
        return current;
    }
    private void acknowledge(long sequence,State state,String error)throws Exception {
        JsonObject report=new JsonObject();report.addProperty("schema",1);report.addProperty("sequence",sequence);
        report.addProperty("mode",state.mode);report.addProperty("alpha",state.alpha);
        report.addProperty("method",state.method);
        if(error!=null)report.addProperty("error",error);
        Path temp=directory.resolve("probe-body-control-result.json.tmp");
        Files.write(temp,report.toString().getBytes(StandardCharsets.UTF_8));
        Files.move(temp,directory.resolve("probe-body-control-result.json"),StandardCopyOption.REPLACE_EXISTING);
    }
}
