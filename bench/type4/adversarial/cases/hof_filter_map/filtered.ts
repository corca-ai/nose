function axis_case(xs: number[]): number[] {
    return xs.filter((x) => x > 0).map((x) => x * 2);
}
