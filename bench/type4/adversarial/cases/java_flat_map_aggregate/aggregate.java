import java.util.Arrays;

class FlatMapAggregate {
    static int javaFlatMapSumLoop(int[] xs, int[] ys) {
        int total = 0;
        for (int x : xs) {
            for (int y : ys) {
                total += x + y;
            }
        }
        return total;
    }

    static int javaFlatMapSum(int[] xs, int[] ys) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(ys).map(y -> x + y))
                .reduce(0, (total, value) -> total + value);
    }

    static int javaFilteredFlatMapSumLoop(int[] xs, int[] ys) {
        int total = 0;
        for (int x : xs) {
            if (x > 0) {
                for (int y : ys) {
                    if (y < 10) {
                        total += x + y;
                    }
                }
            }
        }
        return total;
    }

    static int javaFilteredFlatMapSum(int[] xs, int[] ys) {
        return Arrays.stream(xs)
                .filter(x -> x > 0)
                .flatMap(x -> Arrays.stream(ys).filter(y -> y < 10).map(y -> x + y))
                .reduce(0, (total, value) -> total + value);
    }

    static int javaWrongSeed(int[] xs, int[] ys) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(ys).map(y -> x + y))
                .reduce(1, (total, value) -> total + value);
    }

    static int javaWrongStep(int[] xs, int[] ys) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(ys).map(y -> x + y))
                .reduce(0, (total, value) -> total + value + 1);
    }

    static int javaWrongOuterGuard(int[] xs, int[] ys) {
        return Arrays.stream(xs)
                .filter(x -> x < 0)
                .flatMap(x -> Arrays.stream(ys).filter(y -> y < 10).map(y -> x + y))
                .reduce(0, (total, value) -> total + value);
    }

    static int javaWrongInnerGuard(int[] xs, int[] ys) {
        return Arrays.stream(xs)
                .filter(x -> x > 0)
                .flatMap(x -> Arrays.stream(ys).filter(y -> y > 10).map(y -> x + y))
                .reduce(0, (total, value) -> total + value);
    }

    static int javaFlatMapSource(int[] xs, int[] ys, int[] other) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(ys).map(y -> x + y))
                .reduce(0, (total, value) -> total + value);
    }

    static int javaWrongSource(int[] xs, int[] ys, int[] other) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(other).map(y -> x + y))
                .reduce(0, (total, value) -> total + value);
    }

    static int javaIgnoredInnerSourceYs(int[] xs, int[] ys, int[] other) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(ys).map(y -> x))
                .reduce(0, (total, value) -> total + value);
    }

    static int javaIgnoredInnerSourceOther(int[] xs, int[] ys, int[] other) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(other).map(y -> x))
                .reduce(0, (total, value) -> total + value);
    }

    static int javaSameSourceDirect(int[] xs) {
        return Arrays.stream(xs).reduce(0, (total, value) -> total + value);
    }

    static int javaSameSourceFlatMap(int[] xs) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(xs).map(y -> x + y))
                .reduce(0, (total, value) -> total + value);
    }

    static int javaRecursiveDepthLoop(int[] xs, int[] ys, int[] zs) {
        int total = 0;
        for (int x : xs) {
            for (int y : ys) {
                for (int z : zs) {
                    total += x + y + z;
                }
            }
        }
        return total;
    }

    static int javaRecursiveDepth(int[] xs, int[] ys, int[] zs) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(ys)
                        .flatMap(y -> Arrays.stream(zs).map(z -> x + y + z)))
                .reduce(0, (total, value) -> total + value);
    }

    static int javaRecursiveDepthWrapped(int[] xs, int[] ys, int[] zs) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(ys)
                        .flatMap(y -> Arrays.stream(zs).map(z -> x + y + z))
                        .map(value -> value))
                .reduce(0, (total, value) -> total + value);
    }

    static int javaIgnoredReducerValueYs(int[] xs, int[] ys, int[] other) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(ys).map(y -> x + y))
                .reduce(0, (total, value) -> total + 1);
    }

    static int javaIgnoredReducerValueOther(int[] xs, int[] ys, int[] other) {
        return Arrays.stream(xs)
                .flatMap(x -> Arrays.stream(other).map(y -> x + y))
                .reduce(0, (total, value) -> total + 1);
    }

    static Object javaWrongDepth(int[] xs, int[] ys) {
        return Arrays.stream(xs).map(x -> Arrays.stream(ys).map(y -> x + y));
    }

    static void observe(int value) {}

    static int javaEffectfulCallback(int[] xs, int[] ys) {
        return Arrays.stream(xs)
                .flatMap(x -> {
                    observe(x);
                    return Arrays.stream(ys).map(y -> x + y);
                })
                .reduce(0, (total, value) -> total + value);
    }
}
