package p

func squarePlusOne(value int) int {
	return value*value + 1
}

func AxisCase(x int, y int) int {
	total := x + y
	return squarePlusOne(total)
}
