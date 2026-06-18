int square_plus_one(int value) {
    return value * value + 1;
}

int axis_case(int x, int y) {
    int total = x + y;
    return square_plus_one(total);
}
