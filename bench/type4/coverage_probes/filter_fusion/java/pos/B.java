import java.util.Arrays;

public class B {
    public static Object axisCase(int[] xs) {
        return Arrays.stream(xs).filter(x -> x > 0 && x < 10);
    }
}
