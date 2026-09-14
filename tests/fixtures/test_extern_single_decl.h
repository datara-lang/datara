#ifndef TEST_EXTERN_SINGLE_DECL_H
#define TEST_EXTERN_SINGLE_DECL_H

/* Linkage specification applied to a single declaration (no braces). */

extern "C" long long solo_double(long long v);

/* Plain extern storage qualifier: legacy path, must keep working. */

extern long long solo_plain(long long v);

#endif
