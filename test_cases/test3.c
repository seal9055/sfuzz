#include <stdio.h>
#include <fcntl.h>

int main(int argc, char **argv) {
    volatile int i = 0;
    volatile int count = 0;

    for (int i = 0; i < 100000; i++) {
        count +=1;
    }

    printf("count is: %d", count);
    return 0;
}
