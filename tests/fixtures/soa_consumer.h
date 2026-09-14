#ifndef SOA_CONSUMER_H
#define SOA_CONSUMER_H

typedef struct {
    long long base;
    long long len;
    long long stride;
} SoaComponent;

long long soa_abi_version(void);
long long soa_component_sizeof(void);
long long soa_last_offset(SoaComponent c);
long long soa_negated_len(SoaComponent c);
long long soa_fold_indexed(SoaComponent c, long long (*elem)(long long));
long long soa_fold_scaled(SoaComponent c, long long factor, long long (*elem)(long long));

#endif
