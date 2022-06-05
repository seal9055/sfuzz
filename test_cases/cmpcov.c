#include <stdio.h>
#include <fcntl.h>

int main(int argc, char **argv) {
    char buf[100];
    int fd = open(argv[1], O_RDONLY);

    read(fd, buf, 100);

    if (*(unsigned int*)(buf) == 0xdeadbeef) {
        *(unsigned long*)0x4141414141414141 = 0;
    }

    if (*(unsigned int*)(buf) == 0x4141) {
        *(unsigned long*)0x4141414141414141 = 0;
    }

    return 0;
}
