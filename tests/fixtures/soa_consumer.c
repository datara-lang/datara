#include "soa_consumer.h"

long long soa_abi_version(void)
{
    return 1;
}

long long soa_component_sizeof(void)
{
    return (long long)sizeof(SoaComponent);
}

long long soa_last_offset(SoaComponent c)
{
    if (c.len <= 0) {
        return c.base;
    }
    return c.base + (c.len - 1) * c.stride;
}

long long soa_negated_len(SoaComponent c)
{
    return -c.len;
}

long long soa_fold_indexed(SoaComponent c, long long (*elem)(long long))
{
    long long acc = 0;
    long long i;
    for (i = 0; i < c.len; i = i + 1) {
        acc = acc + elem(i);
    }
    return acc;
}

long long soa_fold_scaled(SoaComponent c, long long factor,
                          long long (*elem)(long long))
{
    long long acc = 0;
    long long i;
    for (i = 0; i < c.len; i = i + 1) {
        acc = acc + elem(i) * factor;
    }
    return acc;
}
