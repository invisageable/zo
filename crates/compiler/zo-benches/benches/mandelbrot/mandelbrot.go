package main

import "fmt"

const (
	WIDTH    = 800
	HEIGHT   = 600
	MAX_ITER = 100
)

func main() {
	width_f := float64(WIDTH)
	height_f := float64(HEIGHT)

	var checksum int64 = 0

	for y := 0; y < HEIGHT; y++ {
		for x := 0; x < WIDTH; x++ {
			cr := float64(x) / width_f * 3.0 - 2.0
			ci := float64(y) / height_f * 2.24 - 1.12

			zr := 0.0
			zi := 0.0
			iter := 0

			for zr * zr + zi * zi <= 4.0 && iter < MAX_ITER {
				temp := zr * zr - zi * zi + cr
				zi = 2.0 * zr * zi + ci
				zr = temp
				iter += 1
			}

			checksum += int64(iter)
		}
	}

	fmt.Println(checksum)
}
