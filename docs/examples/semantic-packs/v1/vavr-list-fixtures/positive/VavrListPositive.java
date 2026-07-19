import io.vavr.collection.List;
import java.util.Arrays;

final class VavrListPositive {
    static boolean externalFactory(int value) {
        return List.of(1, 2, 3, 4, 5).contains(value);
    }

    static boolean builtinFactory(int value) {
        return Arrays.asList(1, 2, 3, 4, 5).contains(value);
    }
}
