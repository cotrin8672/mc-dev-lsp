package io.github.mcdev.core.bytecode.fixtures;

public class LocalCaptureSamples {
    public int instanceWithArgs(String message, int count) {
        int body = count + 1;
        return message.length() + body;
    }

    public static void staticWithWide(long value, double ratio, int flags) {
        staticWithWideMarker(value);
        staticWithWideMarker(ratio);
        staticWithWideMarker(flags);
    }

    private static void staticWithWideMarker(long ignored) {}

    private static void staticWithWideMarker(double ignored) {}

    private static void staticWithWideMarker(int ignored) {}

    public void multipleIndices(int x) {
        int a = x;
        int b = a + 1;
        Math.abs(b);
        int c = b + 1;
        Math.abs(c);
    }
}
