/*
 * Minimal unistd.h shim for MSVC.
 * AOSP libldac includes <unistd.h> but doesn't use any POSIX-only symbols.
 * This empty header satisfies the include on Windows.
 */
#ifndef COMPAT_UNISTD_H
#define COMPAT_UNISTD_H

#include <io.h>
#include <process.h>

#endif /* COMPAT_UNISTD_H */
