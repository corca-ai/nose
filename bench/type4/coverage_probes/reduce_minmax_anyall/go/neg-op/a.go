package p

func SumPositive(xs []int) int {
	total := 0
	for _, x := range xs {
		if x > 0 {
			total = total + x
		}
	}
	return total
}
