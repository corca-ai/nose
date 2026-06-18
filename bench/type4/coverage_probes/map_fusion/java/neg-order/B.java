import java.util.Arrays;

public class B {
    public static Object axisCase(int[] xs) {
        return Arrays.stream(xs).map(x -> x + 1 * 2);
    }
}
