#include <stdio.h>
#include <fcntl.h>

int main(int argc, char **argv) {
    char buf[0x40];
    int fd = open(argv[1], O_RDONLY);

    read(fd, buf, 0x40);

    if (buf[32] == 0x55) {
        //if (buf[8] == 0x4) {
            *(unsigned long*)0x4141414141414141 = 0;
        //}
    }
}
