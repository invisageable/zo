// spectralnorm (benchmarksgame): largest eigenvalue of A[i][j] =
// 1/((i+j)(i+j+1)/2 + i + 1), via 10 power-method iterations of AᵀA
// on the all-ones vector. Serial port. Reads n from argv (default 100).

package main

import (
	"fmt"
	"math"
	"os"
	"strconv"
)

func a(i, j int) float64 {
	return 1.0 / float64((i+j)*(i+j+1)/2+i+1)
}

func multAv(v, out []float64) {
	for i := range out {
		sum := 0.0
		for j, vj := range v {
			sum += a(i, j) * vj
		}
		out[i] = sum
	}
}

func multAtv(v, out []float64) {
	for i := range out {
		sum := 0.0
		for j, vj := range v {
			sum += a(j, i) * vj
		}
		out[i] = sum
	}
}

func multAtAv(v, out, tmp []float64) {
	multAv(v, tmp)
	multAtv(tmp, out)
}

func main() {
	n := 100
	if len(os.Args) > 1 {
		if x, err := strconv.Atoi(os.Args[1]); err == nil {
			n = x
		}
	}

	u := make([]float64, n)
	v := make([]float64, n)
	tmp := make([]float64, n)
	for i := range u {
		u[i] = 1.0
	}

	for i := 0; i < 10; i++ {
		multAtAv(u, v, tmp)
		multAtAv(v, u, tmp)
	}

	vBv, vv := 0.0, 0.0
	for i := 0; i < n; i++ {
		vBv += u[i] * v[i]
		vv += v[i] * v[i]
	}

	fmt.Printf("%0.9f\n", math.Sqrt(vBv/vv))
}
