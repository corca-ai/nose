int c_sum_for(int *xs, int n) {
    int total = 0;
    for (int i = 0; i < n; i = i + 1) {
        total = total + xs[i];
    }
    return total;
}

int c_sum_while(int *ys, int m) {
    int total = 0;
    int i = 0;
    while (i < m) {
        total = total + ys[i];
        i = i + 1;
    }
    return total;
}

int c_count_positive(int *xs, int n) {
    int total = 0;
    for (int i = 0; i < n; i = i + 1) {
        if (xs[i] > 0) {
            total = total + 1;
        }
    }
    return total;
}
