package p
func fac(n int) int {
	if n == 0 {
		return 1
	}
	return n * fac(n-1)
}
