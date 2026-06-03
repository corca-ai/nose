package main

func SumFor(items []int) int {
	total := 0
	for i := 0; i < len(items); i++ {
		total += items[i]
	}
	return total
}

func SumRange(numbers []int) int {
	acc := 0
	for _, n := range numbers {
		acc = acc + n
	}
	return acc
}
