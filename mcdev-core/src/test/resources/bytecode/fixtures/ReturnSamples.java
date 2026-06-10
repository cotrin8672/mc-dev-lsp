package io.github.mcdev.core.bytecode.fixtures;

public class ReturnSamples {
    int returnInt() {
        return 1;
    }

    void returnVoid() {
        return;
    }

    String returnObject() {
        return null;
    }

    int multipleReturns(int x) {
        if (x > 0) {
            return 1;
        }
        return 0;
    }
}
