#ifndef ROXY_ABI_H
#define ROXY_ABI_H

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

enum Errno
#if __STDC_VERSION__ >= 202311L
  : uint64_t
#endif // __STDC_VERSION__ >= 202311L
 {
  Errno_NoSys = 38,
};
#if __STDC_VERSION__ >= 202311L
typedef enum Errno Errno;
#else
typedef uint64_t Errno;
#endif // __STDC_VERSION__ >= 202311L

enum SyscallNumber
#if __STDC_VERSION__ >= 202311L
  : uint64_t
#endif // __STDC_VERSION__ >= 202311L
 {
  SyscallNumber_Exit = 0,
};
#if __STDC_VERSION__ >= 202311L
typedef enum SyscallNumber SyscallNumber;
#else
typedef uint64_t SyscallNumber;
#endif // __STDC_VERSION__ >= 202311L

/**
 * Invokes the Roxy exit syscall and never returns.
 */
void roxy_syscall_exit(uint64_t status);

#endif  /* ROXY_ABI_H */
