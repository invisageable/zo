package main

import "fmt"

const MAX = 440000000

func pow_int(b, e int64) int64 {
	var result int64 = 1
	for i := int64(0); i < e; i++ {
		result *= b
	}
	return result
}

func is_munchhausen(n int64, pwr *[10]int64) bool {
	var sum int64 = 0
	temp := n
	for temp > 0 {
		sum += pwr[temp % 10]
		temp /= 10
	}
	return sum == n
}

func main() {
	var pwr [10]int64
	for i := int64(0); i < 10; i++ {
		pwr[i] = pow_int(i, i)
	}
	for n := int64(1); n <= MAX; n++ {
		if is_munchhausen(n, &pwr) {
			fmt.Println(n)
		}
	}
}
