#include <fcntl.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
  if (argc != 3) return 64;
  if (strcmp(argv[1], "hang") == 0) {
    sleep(5);
    return 0;
  }
  int flags = argv[1][0] == 'r' ? O_RDONLY : O_WRONLY;
  int fd = open(argv[2], flags);
  if (fd >= 0) close(fd);
  return fd >= 0 ? 0 : 1;
}
