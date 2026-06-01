#include <stdio.h>

#define WIDTH 800
#define HEIGHT 600
#define MAX_ITER 100

int main() {
  double width_f = (double)WIDTH;
  double height_f = (double)HEIGHT;

  long checksum = 0;

  for (int y = 0; y < HEIGHT; y++) {
    for (int x = 0; x < WIDTH; x++) {
      double cr = (double)x / width_f * 3.0 - 2.0;
      double ci = (double)y / height_f * 2.24 - 1.12;

      double zr = 0.0;
      double zi = 0.0;
      int iter = 0;

      while (zr * zr + zi * zi <= 4.0 && iter < MAX_ITER) {
        double temp = zr * zr - zi * zi + cr;
        zi = 2.0 * zr * zi + ci;
        zr = temp;
        iter += 1;
      }

      checksum += iter;
    }
  }

  printf("%ld\n", checksum);

  return 0;
}
