package p
func g(n int) int {
	acc := 0
	for n != 0 {
		acc = acc + n
		n = n - 1
	}
	return acc
}
