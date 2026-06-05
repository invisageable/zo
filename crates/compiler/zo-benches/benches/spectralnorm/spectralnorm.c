// spectralnorm (benchmarksgame): largest eigenvalue of the infinite
// matrix A[i][j] = 1/((i+j)(i+j+1)/2 + i + 1), via 10 power-method
// iterations of AᵀA on the all-ones vector. Serial port (the provided
// OpenMP/SSE version uses Linux-only sched.h/cpu_set_t). Prints
// sqrt((u·v)/(v·v)). Reads n from argv (default 100).

#include <stdio.h>
#include <stdlib.h>
#include <math.h>

static double a(int i, int j) {
  return 1.0 / (((i + j) * (i + j + 1) / 2) + i + 1);
}

static void mult_av(const double *v, double *out, int n) {
  for (int i = 0; i < n; i++) {
    double sum = 0.0;
    for (int j = 0; j < n; j++)
      sum += a(i, j) * v[j];
    out[i] = sum;
  }
}

static void mult_atv(const double *v, double *out, int n) {
  for (int i = 0; i < n; i++) {
    double sum = 0.0;
    for (int j = 0; j < n; j++)
      sum += a(j, i) * v[j];
    out[i] = sum;
  }
}

static void mult_atav(const double *v, double *out, double *tmp, int n) {
  mult_av(v, tmp, n);
  mult_atv(tmp, out, n);
}

int main(int argc, char **argv) {
  const int n = argc > 1 ? atoi(argv[1]) : 100;

  double *u = malloc((size_t)n * sizeof(double));
  double *v = malloc((size_t)n * sizeof(double));
  double *tmp = malloc((size_t)n * sizeof(double));

  for (int i = 0; i < n; i++) {
    u[i] = 1.0;
    v[i] = 0.0;
  }

  for (int it = 0; it < 10; it++) {
    mult_atav(u, v, tmp, n);
    mult_atav(v, u, tmp, n);
  }

  double v_bv = 0.0, vv = 0.0;
  for (int i = 0; i < n; i++) {
    v_bv += u[i] * v[i];
    vv += v[i] * v[i];
  }

  printf("%.9f\n", sqrt(v_bv / vv));

  free(u);
  free(v);
  free(tmp);
  return 0;
}
