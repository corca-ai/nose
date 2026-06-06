import java.util.Arrays;
class B { int f(int[] xs) { return Arrays.stream(xs).reduce(1, (a, x) -> a * x); } }
