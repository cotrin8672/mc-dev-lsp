import java.util.*;
/** One-time Java21 formatting oracle. Rust test execution reads only its JSON. */
class FloatingOracle {
    public static void main(String[] args) {
        LinkedHashSet<Integer> floats=new LinkedHashSet<>();
        LinkedHashSet<Long> doubles=new LinkedHashSet<>();
        for(int bits=0;bits<32;bits++){floats.add(bits);floats.add(bits|0x80000000);doubles.add((long)bits);doubles.add(bits|0x8000000000000000L);}
        for(int bits:new int[]{0x00800000,0x007fffff,0x7f7fffff,0x7f800000,0xff800000,0x7fc00000,0x7fa12345,0x3f800000,0x4b189680})floats.add(bits);
        for(long bits:new long[]{0x0010000000000000L,0x000fffffffffffffL,0x7fefffffffffffffL,0x7ff0000000000000L,0xfff0000000000000L,0x7ff8000000000000L,0x7ff0123456789abcL,0x3ff0000000000000L})doubles.add(bits);
        Random random=new Random(0x6006006L);
        for(int i=0;i<1024;i++){floats.add(random.nextInt());doubles.add(random.nextLong());}
        System.out.print("[\n");boolean first=true;
        for(int bits:floats){if(!first)System.out.print(",\n");first=false;System.out.printf("{\"kind\":\"Float\",\"bits\":\"%08x\",\"label\":\"%sf\"}",bits,Float.toString(Float.intBitsToFloat(bits)));}
        for(long bits:doubles){if(!first)System.out.print(",\n");first=false;System.out.printf("{\"kind\":\"Double\",\"bits\":\"%016x\",\"label\":\"%s\"}",bits,Double.toString(Double.longBitsToDouble(bits)));}
        System.out.print("\n]\n");
    }
}
