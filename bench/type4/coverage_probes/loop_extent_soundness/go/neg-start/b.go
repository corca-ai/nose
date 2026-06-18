package p

func AxisCase(xs []int) int {
	total := 0
	for i := 1; i < len(xs); i++ {
		total += xs[i]
	}
	return total
}
