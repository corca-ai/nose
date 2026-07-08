import java.util.Arrays;

class Sum {
    static int javaLoop(int[] xs) {
        int total = 0;
        for (int x : xs) {
            total = total + x;
        }
        return total;
    }

    static int javaReduce(int[] xs) {
        return Arrays.stream(xs).reduce(0, (total, x) -> total + x);
    }

    static int javaProduct(int[] xs) {
        return Arrays.stream(xs).reduce(1, (product, x) -> product * x);
    }
}
