import java.util.Arrays;

public class A {
    public static Object axisCase(int[] xs) {
        return Arrays.stream(xs).filter(x -> x > 0).filter(x -> x < 10);
    }
}
