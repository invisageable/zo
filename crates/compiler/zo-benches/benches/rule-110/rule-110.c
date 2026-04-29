#include <stdio.h>

int main() {
  int width = 31;
  int gens = 15;
  int rule[8] = {0, 1, 1, 1, 0, 1, 1, 0};
  int row[31];
  int next[31];

  for (int i = 0; i < width; i++) {
    row[i] = 0;
  }

  row[width - 1] = 1;

  for (int g = 0; g < gens; g++) {
    for (int p = 0; p < width; p++) {
      if (row[p] == 1) {
        printf("#");
      } else {
        printf(".");
      }
    }
    printf("\n");

    for (int k = 0; k < width; k++) {
      int left = 0;
      if (k > 0) left = row[k - 1];
      int center = row[k];
      int right = 0;
      if (k < width - 1) right = row[k + 1];
      int pattern = left * 4 + center * 2 + right;
      next[k] = rule[pattern];
    }

    for (int i = 0; i < width; i++) {
      row[i] = next[i];
    }
  }

  return 0;
}
