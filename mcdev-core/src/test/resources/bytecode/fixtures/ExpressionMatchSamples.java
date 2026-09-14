package io.github.mcdev.core.bytecode.fixtures;

public class ExpressionMatchSamples {
    int sampleField;

    int readSampleField() {
        return this.sampleField;
    }

    void writeSampleField(int value) {
        this.sampleField = value;
    }

    int add(int a, int b) {
        return a + b;
    }

    void arrayAccess(int[] arr, int index) {
        int value = arr[index];
    }

    void arrayStore(int[] arr, int index, int value) {
        arr[index] = value;
    }

    void assignString() {
        String text = "hello";
    }

    int addAndMultiply(int a, int b, int c) {
        int sum = a + b;
        return sum * c;
    }

    boolean intEqualsZero(int x) {
        if (x == 0) {
            return true;
        }
        return false;
    }

    boolean intComparison(int a, int b) {
        return a == b;
    }

    String stringConcat(int value) {
        return "prefix" + value;
    }

    String trim(String value) {
        return value.trim();
    }

    String newString() {
        return new String();
    }

    String castString(Object value) {
        return (String) value;
    }

    boolean isString(Object value) {
        return value instanceof String;
    }
}
