import java.util.Arrays;

class FlatMap {
    static Object flatMapJava(int[] xs, int[] ys) {
        return Arrays.stream(xs).flatMap(x -> Arrays.stream(ys).map(y -> x + y));
    }

    static Object nestedMapJava(int[] xs, int[] ys) {
        return Arrays.stream(xs).map(x -> Arrays.stream(ys).map(y -> x + y));
    }
}
