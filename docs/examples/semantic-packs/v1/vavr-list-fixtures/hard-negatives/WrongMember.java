import io.vavr.collection.List;

final class WrongMember {
    static boolean externalFactory(int value) {
        return List.ofAll(java.util.List.of(1, 2, 3)).contains(value);
    }

    static boolean builtinFactory(int value) {
        return java.util.List.of(1, 2, 3).contains(value);
    }
}
