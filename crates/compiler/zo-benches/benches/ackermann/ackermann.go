package main

func ack(m, n int) int {
	if m == 0 {
		return n + 1
	}
	if n == 0 {
		return ack(m-1, 1)
	}
	return ack(m-1, ack(m, n-1))
}

func main() {
	if ack(0, 0) != 1 {
		panic("ack(0,0)")
	}
	if ack(3, 2) != 29 {
		panic("ack(3,2)")
	}
	if ack(3, 4) != 125 {
		panic("ack(3,4)")
	}
}
