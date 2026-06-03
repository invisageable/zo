package main

import "fmt"

func main() {
	width := 31
	gens := 15
	rule := [8]int{0, 1, 1, 1, 0, 1, 1, 0}
	row := make([]int, width)
	next := make([]int, width)

	for i := 0; i < width; i++ {
		row[i] = 0
	}

	row[width - 1] = 1

	for g := 0; g < gens; g++ {
		for p := 0; p < width; p++ {
			if row[p] == 1 {
				fmt.Print("#")
			} else {
				fmt.Print(".")
			}
		}
		fmt.Print("\n")

		for k := 0; k < width; k++ {
			left := 0
			if k > 0 {
				left = row[k - 1]
			}
			center := row[k]
			right := 0
			if k < width - 1 {
				right = row[k + 1]
			}
			pattern := left * 4 + center * 2 + right
			next[k] = rule[pattern]
		}

		for i := 0; i < width; i++ {
			row[i] = next[i]
		}
	}
}
