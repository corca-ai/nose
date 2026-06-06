import java.util.Arrays;
class B { int f(int[] xs) { return Arrays.stream(xs).reduce(0, (a, x) -> a + x); } }
