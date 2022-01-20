#include <stdio.h>
#include <stdlib.h>
#include <time.h>

int main() {
    srand(time(NULL));
    int a = rand() % 10;
    int b = 1;

    if (a <= 5) {
        b = b + 20 - a;
    } else {
        b = b + 10 - a;
    }
    printf("%d", b);
    return b;
}
