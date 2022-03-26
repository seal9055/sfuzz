#include <stdio.h>

int main() {
    char buf[8];
    char c;
    for (int i = 0; i < 8; i++) {
        c = buf[i]; 
    }
    printf("%c", c);
}
