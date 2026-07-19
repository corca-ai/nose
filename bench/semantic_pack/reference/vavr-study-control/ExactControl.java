package com.evolutionnext.control;

import io.vavr.collection.List;
import java.util.Arrays;

/** Same-binary exact control for the pinned vavr-study List.of(1..5) call. */
final class ExactControl {
    static boolean vavrFactory(int value) {
        return List.of(1, 2, 3, 4, 5).contains(value);
    }

    static boolean jdkFactory(int value) {
        return Arrays.asList(1, 2, 3, 4, 5).contains(value);
    }
}
