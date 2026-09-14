#ifndef TEST_EXTERN_GUARDED_ONELINE_H
#define TEST_EXTERN_GUARDED_ONELINE_H

/* One-line C++ guard: the directive handler skips the remainder of the
   directive line, so the block braces never reach the declaration parser.
   The stray closing brace after #endif must be tolerated silently. */

#if defined(__cplusplus) extern "C" {
#endif

long long guard_add(long long a, long long b);

#if defined(__cplusplus)
}
#endif

#endif
