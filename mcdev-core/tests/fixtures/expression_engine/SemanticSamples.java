package oracle;

import java.util.function.*;

/** Compiled once by generate_capture.py; Rust tests use the recorded class bytes. */
public class SemanticSamples {
    int field;
    static int shared;
    public int arithmetic(int a, int b, int c) {
        return ((a * b / c % 7 + (a - b)) << 2) ^ ((a >> 1) | (b >>> 3)) & ~c;
    }
    public long longMath(long a, long b) { return -(a + b) ^ -1L; }
    public double floating(double a, double b) { return -a * b + a / b; }
    public boolean intCompare(int a, int b) { return a < b || a >= b || a == b || a != b; }
    public boolean longCompare(long a, long b) { return a <= b; }
    public boolean floatCompare(float a, float b) { return a > b; }
    public boolean doubleCompare(double a, double b) { return a != b; }
    public boolean referenceCompare(Object a, Object b) { return a == b || a == null; }
    public int increments(int value) { int a = value++; int b = ++value; value += 7; return a + b + value; }
    public int loop(int count) { int sum = 0; for(int i=0; i<count; i++) sum += i; return sum; }
    public int table(int value) { return switch(value) { case 1 -> 3; case 2 -> 4; case 3 -> 5; default -> 8; }; }
    public int lookup(int value) { return switch(value) { case -100 -> 3; case 200 -> 4; case 30000 -> 5; default -> 8; }; }
    public String exception(String value) { try { return value.trim(); } catch(RuntimeException error) { throw error; } }
    public Object merge(boolean flag) { Object value; if(flag) value = "value"; else value = new StringBuilder(); return value; }
    public Object[] mergeArrays(boolean flag, String[] a, Integer[] b) { return flag ? a : b; }
    public int[] literal() { return new int[]{1,2,3}; }
    public String[][] multi(int a, int b) { return new String[a][b]; }
    public String[][] partial(int a) { return new String[a][]; }
    public long array(long[] values, int index, long value) { values[index] = value; return values[index]; }
    public int length(Object[] values) { return values.length; }
    public int assignments(int value) { field = value; shared = value; return field + shared; }
    public int boxed(Integer value) { Integer copy = value.intValue(); return copy; }
    public String concat(int value, String text) { return "start:" + value + ":" + text; }
    public String builder(int value, String text) { return new StringBuilder().append("start:").append(value).append(text).toString(); }
    public Function<String,String> bound(String receiver) { return receiver::concat; }
    public Function<String,String> free() { return String::trim; }
    public Supplier<String> constructor() { return String::new; }
    public IntUnaryOperator lambda(int value) { return x -> x + value; }
    public String superCall() { return super.toString(); }
    public Object classLiteral() { return String[][].class; }
    public String unicode() { return "\u0000\uD83D\uDE42\uD800"; }
}
