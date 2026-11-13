package com.arcticlauncher.bedrock;

/** Owned translucent model order, independently porting the cached Java 26.2
 * Mth distance and fastutil 8.5.18 comparator quicksort. This does not invent
 * view poses, foreign submissions or ownership of a native world phase. */
final class JavaModelOrder {
    private JavaModelOrder() {}
    static int[] sort(float[][] translations) {
        if(translations==null||translations.length>2048)throw new IllegalArgumentException("model sorting budget exceeded");
        float[] distances=new float[translations.length];int[] indices=new int[translations.length];
        for(int i=0;i<translations.length;i++) {
            float[] pose=translations[i];
            if(pose==null||pose.length!=3)throw new IllegalArgumentException("model sorting needs a view translation");
            for(float value:pose)if(!Float.isFinite(value))throw new IllegalArgumentException("non-finite model sorting pose");
            float x=pose[0],y=pose[1],z=pose[2];
            distances[i]=(x*x+y*y)+z*z;indices[i]=i;
        }
        quickSort(indices,distances,0,indices.length);return indices;
    }
    private static int compare(int a,int b,float[] distances) {return Float.compare(distances[b],distances[a]);}
    private static void swap(int[] indices,int a,int b) {int value=indices[a];indices[a]=indices[b];indices[b]=value;}
    private static void swap(int[] indices,int a,int b,int count) {for(int i=0;i<count;i++)swap(indices,a+i,b+i);}
    private static int median(int[] indices,float[] distances,int a,int b,int c) {
        int ab=compare(indices[a],indices[b],distances),ac=compare(indices[a],indices[c],distances),bc=compare(indices[b],indices[c],distances);
        return ab<0?(bc<0?b:ac<0?c:a):(bc>0?b:ac>0?c:a);
    }
    private static void quickSort(int[] indices,float[] distances,int from,int to) {
        int length=to-from;
        if(length<16) {
            for(int i=from;i<to-1;i++) {
                int smallest=i;
                for(int j=i+1;j<to;j++)if(compare(indices[j],indices[smallest],distances)<0)smallest=j;
                if(smallest!=i)swap(indices,i,smallest);
            }
            return;
        }
        int left=from,middle=from+length/2,right=to-1;
        if(length>128) {
            int step=length/8;
            left=median(indices,distances,left,left+step,left+2*step);
            middle=median(indices,distances,middle-step,middle,middle+step);
            right=median(indices,distances,right-2*step,right-step,right);
        }
        int pivot=indices[median(indices,distances,left,middle,right)];
        int a=from,b=from,c=to-1,d=to-1;
        while(true) {
            int order;
            while(b<=c&&(order=compare(indices[b],pivot,distances))<=0) {if(order==0)swap(indices,a++,b);b++;}
            while(c>=b&&(order=compare(indices[c],pivot,distances))>=0) {if(order==0)swap(indices,c,d--);c--;}
            if(b>c)break;
            swap(indices,b++,c--);
        }
        int count=Math.min(a-from,b-a);swap(indices,from,b-count,count);
        count=Math.min(d-c,to-d-1);swap(indices,b,to-count,count);
        if(b-a>1)quickSort(indices,distances,from,from+b-a);
        if(d-c>1)quickSort(indices,distances,to-(d-c),to);
    }
}
