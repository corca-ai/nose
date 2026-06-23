final class ImmutableList {
    static Object of(Object value) {
        return value;
    }
}

final class GuavaImmutableCollectionsShadowed {
    Object value() {
        return ImmutableList.of("not-guava");
    }
}
