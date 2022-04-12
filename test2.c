#include <stdio.h>
#include <fcntl.h>

int main(int argc, char **argv) {
    char buf[0x40];
    int fd = open(argv[1], O_RDONLY);

    read(fd, buf, 0x40);

    if (buf[32] == 0x55) {
        puts("AAAAAAAAAAAAAAAAAAAAAAAAAA");
    }
}
