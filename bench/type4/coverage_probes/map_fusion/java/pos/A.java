import java.util.Arrays;

public class A {
    public static Object axisCase(int[] xs) {
        return Arrays.stream(xs).map(x -> x + 1).map(x -> x * 2);
    }
}
