/*
 * bid64_stub.c — Software implementation of Intel BID64 decimal functions.
 *
 * Drop-in replacement for Intel's IntelRDFPMathLib (libbid) for the TWS API.
 * The Decimal type in the TWS API is IEEE 754-2008 64-bit decimal floating
 * point in Binary Integer Decimal (BID) encoding.
 *
 * Encoding layout (standard form, bits 62-61 != 11):
 *   bit  63    : sign (0 = positive)
 *   bits 62-53 : biased exponent  (actual exponent = biased − 398)
 *   bits 52- 0 : unsigned integer coefficient (≤ 9 007 199 254 740 991)
 *
 * This stub covers all values that arise in practice (quantities, prices,
 * commission amounts).  It does NOT implement the "large coefficient"
 * encoding or the full IEEE rounding modes (rmode is accepted but ignored;
 * we always use round-half-up).
 *
 * Build:
 *   gcc -O2 -c bid64_stub.c -o bid64_stub.o
 */

#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <ctype.h>

/* ------------------------------------------------------------------ types */
typedef uint64_t BID64;

/* ---------------------------------------------------------------- constants */
#define SIGN_MASK   UINT64_C(0x8000000000000000)
#define EXP_MASK    UINT64_C(0x7FE0000000000000)
#define COEF_MASK   UINT64_C(0x001FFFFFFFFFFFFF)
#define EXP_SHIFT   53
#define EXP_BIAS    398

/* Maximum coefficient in the standard (non-large) form = 2^53 − 1 */
#define MAX_COEF    UINT64_C(9007199254740991)

/* Canonical representations */
#define BID64_PZERO UINT64_C(0x31C0000000000000)   /* +0E+0            */
#define BID64_NAN   UINT64_C(0x7C00000000000000)   /* quiet NaN        */
#define BID64_PINF  UINT64_C(0x7800000000000000)   /* +Infinity        */

/* ------------------------------------------------------------ helpers */

static int bid64_is_nan(BID64 v)
{
    /* quiet NaN: bits 62-58 = 11111 */
    return (v & UINT64_C(0x7C00000000000000)) == UINT64_C(0x7C00000000000000);
}

static int bid64_is_inf(BID64 v)
{
    return (v & UINT64_C(0x7C00000000000000)) == BID64_PINF;
}

/*
 * Encode a (sign, coefficient, exponent) triple into BID64.
 * Normalises the coefficient/exponent pair so the coefficient fits in
 * MAX_COEF.  Returns BID64_NAN on overflow.
 */
static BID64 bid64_encode(int sign, uint64_t coef, int exp)
{
    /* Zero */
    if (coef == 0) {
        if (exp < -EXP_BIAS) exp = -EXP_BIAS;
        if (exp >  369)      exp =  369;
        return ((uint64_t)(sign & 1) << 63)
             | ((uint64_t)(exp + EXP_BIAS) << EXP_SHIFT);
    }

    /* Reduce coefficient that is too large */
    while (coef > MAX_COEF) {
        if (exp >= 369) return sign ? (BID64_PINF | SIGN_MASK) : BID64_PINF;
        coef = (coef + 5) / 10;   /* round-half-up */
        exp++;
    }

    /* Increase exponent to fit in the allowed range */
    while (exp < -EXP_BIAS) {
        if (coef == 0) break;
        coef /= 10;
        exp++;
    }

    if (exp < -EXP_BIAS) exp = -EXP_BIAS;
    if (exp >  369)      exp =  369;

    return ((uint64_t)(sign & 1) << 63)
         | ((uint64_t)(exp + EXP_BIAS) << EXP_SHIFT)
         | (coef & COEF_MASK);
}

/*
 * Decode a BID64 value.  Returns 0 for NaN/Inf, 1 otherwise.
 */
static int bid64_decode(BID64 v, int *sign, uint64_t *coef, int *exp)
{
    if (bid64_is_nan(v) || bid64_is_inf(v)) return 0;

    *sign = (int)((v & SIGN_MASK) >> 63);

    uint64_t bits62_61 = (v >> 61) & 0x3;
    if (bits62_61 == 0x3) {
        /* "large coefficient" form — coefficient = 2^53 + lower 51 bits,
         * exponent from bits 62-51 (upper 2 from bits 60-59). */
        uint64_t exp_upper = (v >> 59) & 0x3;
        uint64_t exp_lower = (v >> 51) & 0xFF;
        *exp  = (int)((exp_upper << 8) | exp_lower) - EXP_BIAS;
        *coef = UINT64_C(0x0020000000000000) | (v & UINT64_C(0x0007FFFFFFFFFFFF));
    } else {
        *exp  = (int)((v & EXP_MASK) >> EXP_SHIFT) - EXP_BIAS;
        *coef = v & COEF_MASK;
    }
    return 1;
}

/*
 * Align two operands to the same exponent (in-place).
 * The operand with the larger exponent has its coefficient scaled down.
 */
static void bid64_align(uint64_t *c1, int *e1, uint64_t *c2, int *e2)
{
    /* Scale the smaller-exponent operand up first (no precision loss) */
    while (*e1 > *e2 && *c1 <= MAX_COEF / 10) { *c1 *= 10; (*e1)--; }
    while (*e2 > *e1 && *c2 <= MAX_COEF / 10) { *c2 *= 10; (*e2)--; }

    /* Scale the larger-exponent operand down (lossy) */
    while (*e1 > *e2) { *c1 /= 10; (*e1)--; }
    while (*e2 > *e1) { *c2 /= 10; (*e2)--; }
}

/* ============================================================= public API */

BID64 __bid64_add(BID64 a, BID64 b, unsigned int rmode, unsigned int *flags)
{
    *flags = 0;
    if (bid64_is_nan(a) || bid64_is_nan(b)) return BID64_NAN;

    int sa, sb;
    uint64_t ca, cb;
    int ea, eb;

    if (!bid64_decode(a, &sa, &ca, &ea)) return BID64_NAN;
    if (!bid64_decode(b, &sb, &cb, &eb)) return BID64_NAN;

    bid64_align(&ca, &ea, &cb, &eb);

    int sr;
    uint64_t rc;

    if (sa == sb) {
        rc = ca + cb;
        sr = sa;
    } else {
        if (ca >= cb) { rc = ca - cb; sr = sa; }
        else          { rc = cb - ca; sr = sb; }
    }

    return bid64_encode(sr, rc, ea);
}

BID64 __bid64_sub(BID64 a, BID64 b, unsigned int rmode, unsigned int *flags)
{
    *flags = 0;
    /* Negate b's sign bit and add */
    BID64 neg_b = bid64_is_nan(b) ? b : (b ^ SIGN_MASK);
    return __bid64_add(a, neg_b, rmode, flags);
}

BID64 __bid64_mul(BID64 a, BID64 b, unsigned int rmode, unsigned int *flags)
{
    *flags = 0;
    if (bid64_is_nan(a) || bid64_is_nan(b)) return BID64_NAN;

    int sa, sb;
    uint64_t ca, cb;
    int ea, eb;

    if (!bid64_decode(a, &sa, &ca, &ea)) return BID64_NAN;
    if (!bid64_decode(b, &sb, &cb, &eb)) return BID64_NAN;

    int sr = sa ^ sb;
    int re = ea + eb;

    /* Use 128-bit arithmetic to avoid overflow */
    __uint128_t result = (__uint128_t)ca * cb;

    while (result > MAX_COEF) {
        if (re >= 369) return sr ? (BID64_PINF | SIGN_MASK) : BID64_PINF;
        result = (result + 5) / 10;
        re++;
    }

    return bid64_encode(sr, (uint64_t)result, re);
}

BID64 __bid64_div(BID64 a, BID64 b, unsigned int rmode, unsigned int *flags)
{
    *flags = 0;
    if (bid64_is_nan(a) || bid64_is_nan(b)) return BID64_NAN;

    int sa, sb;
    uint64_t ca, cb;
    int ea, eb;

    if (!bid64_decode(a, &sa, &ca, &ea)) return BID64_NAN;
    if (!bid64_decode(b, &sb, &cb, &eb)) return BID64_NAN;

    if (cb == 0) return BID64_NAN;   /* division by zero → NaN */

    int sr = sa ^ sb;
    int re = ea - eb;

    /* Scale numerator up by as many powers of 10 as possible to retain
     * up to 16 significant digits in the result. */
    __uint128_t num = (__uint128_t)ca;
    while (num < (__uint128_t)cb * UINT64_C(1000000000000000) && re > -EXP_BIAS) {
        num *= 10;
        re--;
    }

    uint64_t rc = (uint64_t)(num / cb);
    return bid64_encode(sr, rc, re);
}

double __bid64_to_binary64(BID64 a, unsigned int rmode, unsigned int *flags)
{
    *flags = 0;
    if (bid64_is_nan(a)) return 0.0 / 0.0;
    if (bid64_is_inf(a)) return (a & SIGN_MASK) ? -1.0 / 0.0 : 1.0 / 0.0;

    int sa;
    uint64_t ca;
    int ea;

    if (!bid64_decode(a, &sa, &ca, &ea)) return 0.0 / 0.0;

    double result = (double)ca * pow(10.0, (double)ea);
    return sa ? -result : result;
}

BID64 __binary64_to_bid64(double d, unsigned int rmode, unsigned int *flags)
{
    *flags = 0;
    if (isnan(d))  return BID64_NAN;
    if (isinf(d))  return d > 0 ? BID64_PINF : (BID64_PINF | SIGN_MASK);

    int sign = (d < 0.0);
    if (sign) d = -d;
    if (d == 0.0)  return bid64_encode(sign, 0, 0);

    /* Scale d into a 15-digit integer */
    double lg = floor(log10(d));
    int exp = (int)lg - 14;                     /* target: 15-digit coefficient */
    double scaled = d * pow(10.0, -exp);
    uint64_t coef = (uint64_t)(scaled + 0.5);   /* round to nearest */

    return bid64_encode(sign, coef, exp);
}

BID64 __bid64_from_string(char *str, unsigned int rmode, unsigned int *flags)
{
    *flags = 0;
    if (!str || *str == '\0') return BID64_PZERO;

    char *p = str;
    int sign = 0;

    /* sign */
    if (*p == '-') { sign = 1; p++; }
    else if (*p == '+') { p++; }

    /* special values */
    if (strncasecmp(p, "nan",  3) == 0) return BID64_NAN;
    if (strncasecmp(p, "inf",  3) == 0) return sign ? (BID64_PINF | SIGN_MASK) : BID64_PINF;

    uint64_t coef = 0;
    int exp = 0;
    int dec_seen = 0;   /* have we seen the decimal point? */
    int digits = 0;     /* significant digits accumulated  */

    for (; *p && *p != 'E' && *p != 'e'; p++) {
        if (*p == '.') {
            dec_seen = 1;
        } else if (*p >= '0' && *p <= '9') {
            if (digits < 16) {
                coef = coef * 10 + (uint64_t)(*p - '0');
                digits++;
                if (dec_seen) exp--;    /* shift exponent for fractional digit */
            } else {
                /* beyond 16 significant digits: track exponent shift only */
                if (!dec_seen) exp++;
                /* fractional tail is simply dropped (truncated) */
            }
        }
    }

    /* optional exponent */
    if (*p == 'E' || *p == 'e') {
        p++;
        int esign = 1;
        if (*p == '+') { p++; }
        else if (*p == '-') { esign = -1; p++; }
        int eval = 0;
        while (*p >= '0' && *p <= '9') { eval = eval * 10 + (*p++ - '0'); }
        exp += esign * eval;
    }

    return bid64_encode(sign, coef, exp);
}

void __bid64_to_string(char *str, BID64 a, unsigned int *flags)
{
    *flags = 0;

    if (bid64_is_nan(a)) { strcpy(str, "+NaN"); return; }
    if (bid64_is_inf(a)) {
        strcpy(str, (a & SIGN_MASK) ? "-Inf" : "+Inf");
        return;
    }

    int sa;
    uint64_t ca;
    int ea;

    if (!bid64_decode(a, &sa, &ca, &ea)) { strcpy(str, "+NaN"); return; }

    if (ca == 0) {
        /* Represent as "+0E+0" to match TWS API expectations */
        sprintf(str, "%s0E+0", sa ? "-" : "+");
        return;
    }

    /* Format coefficient as decimal string */
    char coef_buf[24];
    sprintf(coef_buf, "%llu", (unsigned long long)ca);
    int num_digits = (int)strlen(coef_buf);

    /*
     * Output in engineering-style scientific notation: ±D.DDDDEsNN
     * as produced by the Intel library (e.g. "+1.5E+2" for 150).
     */
    char *out = str;
    *out++ = sa ? '-' : '+';
    *out++ = coef_buf[0];

    if (num_digits > 1) {
        *out++ = '.';
        /* Copy remaining digits, stripping trailing zeros */
        int last_nonzero = num_digits - 1;
        while (last_nonzero > 0 && coef_buf[last_nonzero] == '0') last_nonzero--;
        for (int i = 1; i <= last_nonzero; i++) *out++ = coef_buf[i];
    }

    /* Exponent: value × 10^(ea + num_digits − 1) */
    int display_exp = ea + num_digits - 1;
    sprintf(out, "E%+d", display_exp);
}
