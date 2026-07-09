package clampbridge

import "cmp"

func clampGoMinMaxGuarded(x int, lo int, hi int) int {
	if hi < lo {
		return lo
	}
	return min(max(x, lo), hi)
}

func clampGoMaxMinGuarded(x int, lo int, hi int) int {
	if hi < lo {
		return lo
	}
	return max(min(x, hi), lo)
}

func clampGoMaxMinUnproven(x int, lo int, hi int) int {
	return max(min(x, hi), lo)
}

func clampGoSwappedBounds(x int, lo int, hi int) int {
	if hi < lo {
		return lo
	}
	return min(max(x, hi), lo)
}

func clampGoWrongNesting(x int, lo int, hi int) int {
	if hi < lo {
		return lo
	}
	return max(min(x, lo), hi)
}

func clampGoFloatGuarded(x float64, lo float64, hi float64) float64 {
	if hi < lo {
		return lo
	}
	return max(min(x, hi), lo)
}

func clampGoGenericOrdered[T cmp.Ordered](val T, minimum T, maximum T) T {
	return max(min(val, maximum), minimum)
}

func clampGoGenericOrderedGuarded[T cmp.Ordered](val T, minimum T, maximum T) T {
	if maximum < minimum {
		return minimum
	}
	return max(min(val, maximum), minimum)
}
