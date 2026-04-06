/*
 * MSVC compatibility header for AOSP libldac.
 * Force-included before all source files to patch GCC-isms.
 */
#ifndef LDAC_COMPAT_H
#define LDAC_COMPAT_H

#ifdef _MSC_VER
/* GCC __attribute__ is not supported by MSVC */
#ifndef __attribute__
#define __attribute__(x)
#endif
#endif

#endif /* LDAC_COMPAT_H */
