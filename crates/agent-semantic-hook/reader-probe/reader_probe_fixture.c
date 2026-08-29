#include <fcntl.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
  if (argc != 3) return 64;
  if (strcmp(argv[1], "hang") == 0) {
    sleep(5);
    return 0;
  }
  int flags;
  if (strcmp(argv[1], "read") == 0) {
    flags = O_RDONLY;
  } else if (strcmp(argv[1], "write") == 0) {
    flags = O_WRONLY;
  } else if (strcmp(argv[1], "read-write") == 0) {
    flags = O_RDWR;
  } else {
    return 65;
  }
  int fd = open(argv[2], flags);
  if (fd >= 0) close(fd);
  return fd >= 0 ? 0 : 1;
}
