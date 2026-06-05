// fannkuch-redux (benchmarksgame): for every permutation of n
// elements, flip the prefix whose length is the first element until
// that element is 0, counting flips. Reports the maximum flip count
// and a sign-alternating checksum. Serial port. Reads n from argv
// (default 7).

package main

import (
	"fmt"
	"os"
	"strconv"
)

func fannkuch(n int) (int, int) {
	current := make([]int, n+1)
	temp := make([]int, n+1)
	count := make([]int, n+1)

	for i := 0; i < n; i++ {
		current[i] = i
	}

	permMax := 1
	for i := 1; i <= n; i++ {
		permMax *= i
	}

	maxFlips := 0
	checksum := 0

	for permIndex := 0; ; permIndex++ {
		if current[0] > 0 {
			copy(temp, current)

			flipCount := 1
			firstValue := current[0]

			for temp[firstValue] != 0 {
				newFirst := temp[firstValue]
				temp[firstValue] = firstValue

				if firstValue > 2 {
					for lo, hi := 1, firstValue-1; lo < hi; lo, hi = lo+1, hi-1 {
						temp[lo], temp[hi] = temp[hi], temp[lo]
					}
				}

				firstValue = newFirst
				flipCount++
			}

			if permIndex%2 == 0 {
				checksum += flipCount
			} else {
				checksum -= flipCount
			}

			if flipCount > maxFlips {
				maxFlips = flipCount
			}
		}

		if permIndex >= permMax-1 {
			break
		}

		// next permutation — a factorial-radix increment of count.
		firstValue := current[1]
		current[1] = current[0]
		current[0] = firstValue

		i := 1
		for count[i] >= i {
			count[i] = 0
			i++

			newFirst := current[1]
			current[0] = newFirst

			for j := 1; j < i; j++ {
				current[j] = current[j+1]
			}

			current[i] = firstValue
			firstValue = newFirst
		}

		count[i]++
	}

	return checksum, maxFlips
}

func main() {
	n := 7
	if len(os.Args) > 1 {
		if v, err := strconv.Atoi(os.Args[1]); err == nil {
			n = v
		}
	}

	checksum, maxFlips := fannkuch(n)

	fmt.Printf("%d\nPfannkuchen(%d) = %d\n", checksum, n, maxFlips)
}
