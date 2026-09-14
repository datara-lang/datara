#ifndef TEST_EXTERN_BLOCK_RUNTIME_H
#define TEST_EXTERN_BLOCK_RUNTIME_H

/* Block form wrapping a Datara runtime C declaration. The JIT resolves
   datara_rt_* symbols from its runtime registry, so this fixture runs via
   `forgen run --jit` on hosts without a native C/C++ toolchain. */

#ifdef __cplusplus
extern "C" {
#endif

double datara_rt_time_precise_ms(void);

#ifdef __cplusplus
}
#endif

#endif
