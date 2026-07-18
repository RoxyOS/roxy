#include <stddef.h>
#include <stdint.h>

#include "roxy/abi.h"

_Static_assert(sizeof(SyscallNumber) == 8, "SyscallNumber size changed");
_Static_assert(_Alignof(SyscallNumber) == 8, "SyscallNumber alignment changed");
_Static_assert(sizeof(Errno) == 8, "Errno size changed");
_Static_assert(_Alignof(Errno) == 8, "Errno alignment changed");
static void (*const exit_wrapper)(uint64_t) = roxy_syscall_exit;

int main(void) {
    return exit_wrapper == NULL;
}
