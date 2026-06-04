/* threadring (benchmarksgame) — C / pthreads.

   503 threads in a ring, each holding a mutex used as a baton.
   A node's run() blocks on its own mutex; the previous node's
   put() sets the value and unlocks it, handing over the token.
   When the value reaches 0 the holder prints its label and
   exits the process. Mirrors the reference mutex variant. */

#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#define NTHREADS 503

typedef struct T {
   struct T *next;
   int label;
   int value;
   pthread_mutex_t mux;
} T;

static T channels[NTHREADS];

static void put(T *w, int v) {
   w->value = v;
   if (v == 0) {
      printf("%d\n", w->label);
      exit(0);
   }
   pthread_mutex_unlock(&w->mux);
}

static void *run(void *arg) {
   T *w = (T *)arg;
   for (;;) {
      pthread_mutex_lock(&w->mux);
      put(w->next, w->value - 1);
   }
   return NULL;
}

int main(int argc, char **argv) {
   int n = 1000;
   if (argc > 1) {
      n = atoi(argv[1]);
   }

   for (int i = 0; i < NTHREADS; i++) {
      channels[i].label = i + 1;
      channels[i].next = &channels[(i + 1) % NTHREADS];
      pthread_mutex_init(&channels[i].mux, NULL);
      pthread_mutex_lock(&channels[i].mux);

      pthread_t tid;
      pthread_create(&tid, NULL, run, &channels[i]);
      pthread_detach(tid);
   }

   put(&channels[0], n);

   for (;;) {
      pause();
   }
   return 0;
}
