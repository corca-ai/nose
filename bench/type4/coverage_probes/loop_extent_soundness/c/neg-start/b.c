int axis_case(int *xs, int n) {
    int total = 0;
    for (int i = 1; i < n; i++) {
        total += xs[i];
    }
    return total;
}
