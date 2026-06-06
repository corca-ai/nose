class B { int f(int[] xs) { return java.util.Arrays.stream(xs).reduce(1, (a, x) -> a * x); } }
