#ifndef COMMON_COMMON_SYMBOLS_H
#define COMMON_COMMON_SYMBOLS_H

#ifndef COMMON_PREFIX
#error "COMMON_PREFIX must be defined by the consumer"
#endif

#define COMMON_CAT_IMPL(prefix, name) prefix ## name
#define COMMON_CAT(prefix, name) COMMON_CAT_IMPL(prefix, name)

#define fft COMMON_CAT(COMMON_PREFIX, fft)
#define fft_init COMMON_CAT(COMMON_PREFIX, fft_init)
#define num_sfbs_1024 COMMON_CAT(COMMON_PREFIX, num_sfbs_1024)
#define num_sfbs_128 COMMON_CAT(COMMON_PREFIX, num_sfbs_128)
#define qmf_c COMMON_CAT(COMMON_PREFIX, qmf_c)
#define sbr_offset COMMON_CAT(COMMON_PREFIX, sbr_offset)
#define sfb_offsets_1024 COMMON_CAT(COMMON_PREFIX, sfb_offsets_1024)
#define sfb_offsets_128 COMMON_CAT(COMMON_PREFIX, sfb_offsets_128)

#endif
