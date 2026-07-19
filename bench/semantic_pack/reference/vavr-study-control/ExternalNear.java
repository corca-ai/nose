package com.evolutionnext.control;

import io.vavr.collection.List;

final class ExternalNear {
    Object collect(Object first, Object second, Object third) {
        Object values = List.of(first, second, third);
        int size = first.hashCode() + second.hashCode();
        if (size > 0) {
            return values;
        }
        return List.of(third, second, first);
    }
}
