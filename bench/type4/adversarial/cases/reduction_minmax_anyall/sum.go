package p

func GoSumRange(xs []int) int {
	total := 0
	for _, x := range xs {
		total = total + x
	}
	return total
}

func GoSumIndex(xs []int) int {
	total := 0
	i := 0
	for i < len(xs) {
		total = total + xs[i]
		i = i + 1
	}
	return total
}

func GoCountPositive(xs []int) int {
	total := 0
	for _, x := range xs {
		if x > 0 {
			total = total + 1
		}
	}
	return total
}
