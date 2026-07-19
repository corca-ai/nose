import io.vavr.collection.List;

final class WrongArity {
    static boolean externalFactory(int value) {
        return List.of(1, 2).contains(value);
    }

    static boolean builtinFactory(int value) {
        return java.util.List.of(1, 2, 3).contains(value);
    }
}
