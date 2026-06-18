package p

func SumPositive(xs []int) int {
	total := 0
	i := 0
	for i < len(xs) {
		if xs[i] > 0 {
			total = total + xs[i]
		}
		i = i + 1
	}
	return total
}
