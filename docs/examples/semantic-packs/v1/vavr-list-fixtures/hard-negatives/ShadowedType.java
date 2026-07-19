import io.vavr.collection.List;

final class ShadowedType {
    static final class List {
        static java.util.List<Integer> of(int first, int second, int third) {
            return java.util.List.of(first, second, third);
        }
    }

    static boolean externalFactory(int value) {
        return List.of(1, 2, 3).contains(value);
    }

    static boolean builtinFactory(int value) {
        return java.util.List.of(1, 2, 3).contains(value);
    }
}
