#include <dlfcn.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

extern int probe_target_open_nocancel(const char *, int, ...)
    __asm("_open$NOCANCEL");
extern int probe_target_openat_nocancel(int, const char *, int, ...)
    __asm("_openat$NOCANCEL");

static const char *probe_target;
static int probe_fd = -1;
typedef int (*probe_open_fn)(const char *, int, ...);
typedef int (*probe_openat_fn)(int, const char *, int, ...);
static probe_open_fn probe_original_open;
static probe_openat_fn probe_original_openat;

__attribute__((constructor)) static void initialize_probe(void) {
  probe_target = getenv("ASP_READER_PROBE_TARGET");
  const char *fd = getenv("ASP_READER_PROBE_FD");
  if (fd != NULL)
    probe_fd = atoi(fd);
  probe_original_open = (probe_open_fn)dlsym(RTLD_NEXT, "open");
  probe_original_openat = (probe_openat_fn)dlsym(RTLD_NEXT, "openat");
}

static void report_target_open(const char *path, int flags) {
  if (probe_fd < 0 || probe_target == NULL || path == NULL ||
      strcmp(path, probe_target) != 0)
    return;
  (void)write(probe_fd, &flags, sizeof(flags));
  _exit(86);
}

static int open_mode(int flags, va_list args) {
  return (flags & O_CREAT) != 0 ? va_arg(args, int) : 0;
}

static int replacement_open(const char *path, int flags, ...) {
  va_list args;
  va_start(args, flags);
  int mode = open_mode(flags, args);
  va_end(args);
  report_target_open(path, flags);
  if (probe_original_open == NULL)
    return probe_target_open_nocancel(path, flags, mode);
  return probe_original_open(path, flags, mode);
}

static int replacement_openat(int directory, const char *path, int flags, ...) {
  va_list args;
  va_start(args, flags);
  int mode = open_mode(flags, args);
  va_end(args);
  report_target_open(path, flags);
  if (probe_original_openat == NULL)
    return probe_target_openat_nocancel(directory, path, flags, mode);
  return probe_original_openat(directory, path, flags, mode);
}

#define DYLD_INTERPOSE(replacement, replacee)                                  \
  __attribute__((used)) static struct {                                        \
    const void *replacement;                                                   \
    const void *replacee;                                                      \
  } _interpose_##replacement##replacee                                         \
      __attribute__((section("__DATA,__interpose"))) = {                       \
          (const void *)(unsigned long)&replacement,                           \
          (const void *)(unsigned long)&replacee};

DYLD_INTERPOSE(replacement_open, open)
DYLD_INTERPOSE(replacement_openat, openat)
