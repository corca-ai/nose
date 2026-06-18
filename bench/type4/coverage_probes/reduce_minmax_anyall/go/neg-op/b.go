package p

func CountPositive(xs []int) int {
	total := 0
	i := 0
	for i < len(xs) {
		if xs[i] > 0 {
			total = total + 1
		}
		i = i + 1
	}
	return total
}
