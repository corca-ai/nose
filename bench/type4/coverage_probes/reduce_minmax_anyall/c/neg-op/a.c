int sum_positive(int *xs, int n) {
    int total = 0;
    for (int i = 0; i < n; i = i + 1) {
        if (xs[i] > 0) {
            total = total + xs[i];
        }
    }
    return total;
}
