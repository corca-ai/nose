package com.evolutionnext.control;

import java.util.Arrays;

final class BuiltinNear {
    Object gather(Object first, Object second, Object third) {
        Object values = Arrays.asList(first, second, third);
        int size = first.hashCode() + second.hashCode();
        if (size > 0) {
            return values;
        }
        return Arrays.asList(first, second, third, null);
    }
}
