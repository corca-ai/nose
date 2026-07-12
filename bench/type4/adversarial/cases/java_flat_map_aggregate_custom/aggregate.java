import java.util.Arrays;

class FlatMapAggregateCustom {
    static int javaNestedLoopReference(int[] xs, int[] ys) {
        int total = 0;
        for (int x : xs) {
            for (int y : ys) {
                total += x + y;
            }
        }
        return total;
    }

    static int javaCustomDispatch(int[] xs, int[] ys) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(ys).map(y -> x + y))
                .reduce(0, (total, value) -> total + value);
    }
}

class Arrays {}
