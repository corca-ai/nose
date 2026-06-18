int count_positive(int *xs, int n) {
    int total = 0;
    int i = 0;
    while (i < n) {
        if (xs[i] > 0) {
            total = total + 1;
        }
        i = i + 1;
    }
    return total;
}
