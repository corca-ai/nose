package p
func fac(n int) int {
	acc := 1
	for n != 0 {
		acc = acc * n
		n = n - 1
	}
	return acc
}
