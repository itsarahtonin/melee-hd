//! The Metrowerks standard library's formatted output (`MSL/printf.c`, `ansi_fp.c`), as the
//! debug text screens print through it (`DevText_Printf`'s `vsnprintf`): its decimal
//! conversion of doubles and its rounding, which a debug screen's glyphs show.

use gekko_fp as fp;

/// An argument `__pformatter` takes from the list: an `int`, a `double` (a float passed
/// through `...` is promoted to one) or a string.
#[derive(Clone, Copy, Debug)]
pub enum Arg<'a> {
    Int(i32),
    Float(f64),
    Str(&'a [u8]),
}

/// `print_format`'s `justification_options`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Justify {
    Left,
    Right,
    ZeroFill,
}

/// `print_format`'s `sign_options`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sign {
    OnlyMinus,
    Always,
    Space,
}

/// `print_format`'s `argument_options`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ArgKind {
    Normal,
    Char,
    Short,
    Long,
    LongLong,
    LongDouble,
    Wchar,
}

/// `print_format`.
#[derive(Clone, Copy)]
struct Format {
    justify: Justify,
    sign: Sign,
    precision_specified: bool,
    alternate_form: bool,
    arg: ArgKind,
    conversion: u8,
    field_width: i32,
    precision: i32,
}

/// `conversion_char`'s value for a format it can't convert.
const BAD: u8 = 0xFF;

/// The arguments as `va_arg` takes them, in order.
struct Args<'a, 'b> {
    list: &'b [Arg<'a>],
    next: usize,
}

impl<'a> Args<'a, '_> {
    fn take(&mut self) -> Arg<'a> {
        let a = *self
            .list
            .get(self.next)
            .unwrap_or_else(|| panic!("a format takes more arguments than its call passes"));
        self.next += 1;
        a
    }

    fn int(&mut self) -> i32 {
        match self.take() {
            Arg::Int(i) => i,
            a => panic!("an integer conversion of {a:?}"),
        }
    }

    fn float(&mut self) -> f64 {
        match self.take() {
            Arg::Float(f) => f,
            a => panic!("a float conversion of {a:?}"),
        }
    }

    fn str(&mut self) -> &'a [u8] {
        match self.take() {
            Arg::Str(s) => s,
            a => panic!("a string conversion of {a:?}"),
        }
    }
}

/// The byte at `i` of a format string, 0 past its end (the string's terminator).
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

/// `parse_format`: the conversion at `s` (a `%`), and where the format goes on.
fn parse_format(fmt: &[u8], mut s: usize, args: &mut Args) -> (Format, usize) {
    let mut f = Format {
        justify: Justify::Right,
        sign: Sign::OnlyMinus,
        precision_specified: false,
        alternate_form: false,
        arg: ArgKind::Normal,
        conversion: 0,
        field_width: 0,
        precision: 0,
    };
    s += 1;
    let mut c = at(fmt, s);
    if c == b'%' {
        f.conversion = c;
        return (f, s + 1);
    }
    loop {
        match c {
            b'-' => f.justify = Justify::Left,
            b'+' => f.sign = Sign::Always,
            b' ' => {
                if f.sign != Sign::Always {
                    f.sign = Sign::Space;
                }
            }
            b'#' => f.alternate_form = true,
            b'0' => {
                if f.justify != Justify::Left {
                    f.justify = Justify::ZeroFill;
                }
            }
            _ => break,
        }
        s += 1;
        c = at(fmt, s);
    }
    if c == b'*' {
        f.field_width = args.int();
        if f.field_width < 0 {
            f.justify = Justify::Left;
            f.field_width = f.field_width.wrapping_neg();
        }
        s += 1;
        c = at(fmt, s);
    } else {
        while c.is_ascii_digit() {
            f.field_width = f
                .field_width
                .wrapping_mul(10)
                .wrapping_add(i32::from(c - b'0'));
            s += 1;
            c = at(fmt, s);
        }
    }
    if f.field_width > 509 {
        f.conversion = BAD;
        return (f, s + 1);
    }
    if c == b'.' {
        f.precision_specified = true;
        s += 1;
        c = at(fmt, s);
        if c == b'*' {
            f.precision = args.int();
            if f.precision < 0 {
                f.precision_specified = false;
            }
            s += 1;
            c = at(fmt, s);
        } else {
            while c.is_ascii_digit() {
                f.precision = f
                    .precision
                    .wrapping_mul(10)
                    .wrapping_add(i32::from(c - b'0'));
                s += 1;
                c = at(fmt, s);
            }
        }
    }
    let mut found = true;
    match c {
        b'h' => {
            f.arg = ArgKind::Short;
            if at(fmt, s + 1) == b'h' {
                f.arg = ArgKind::Char;
                s += 1;
            }
        }
        b'l' => {
            f.arg = ArgKind::Long;
            if at(fmt, s + 1) == b'l' {
                f.arg = ArgKind::LongLong;
                s += 1;
            }
        }
        b'L' => f.arg = ArgKind::LongDouble,
        _ => found = false,
    }
    if found {
        s += 1;
        c = at(fmt, s);
    }
    f.conversion = c;
    match c {
        b'd' | b'i' | b'u' | b'o' | b'x' | b'X' => {
            if f.arg == ArgKind::LongDouble {
                f.conversion = BAD;
            } else if !f.precision_specified {
                f.precision = 1;
            } else if f.justify == Justify::ZeroFill {
                f.justify = Justify::Right;
            }
        }
        b'f' => {
            if f.arg == ArgKind::Short || f.arg == ArgKind::LongLong {
                f.conversion = BAD;
            } else if !f.precision_specified {
                f.precision = 6;
            }
        }
        b'g' | b'G' | b'e' | b'E' => {
            if (c == b'g' || c == b'G') && f.precision == 0 {
                f.precision = 1;
            }
            if f.arg == ArgKind::Short || f.arg == ArgKind::LongLong || f.arg == ArgKind::Char {
                f.conversion = BAD;
            } else if !f.precision_specified {
                f.precision = 6;
            }
        }
        b'p' => {
            f.conversion = b'x';
            f.alternate_form = true;
            f.arg = ArgKind::Long;
            f.precision = 8;
        }
        b'c' | b's' => {
            if f.arg == ArgKind::Long {
                f.arg = ArgKind::Wchar;
            } else if (c == b'c' && f.precision_specified) || f.arg != ArgKind::Normal {
                f.conversion = BAD;
            }
        }
        b'n' => {
            if f.arg == ArgKind::LongDouble {
                f.conversion = BAD;
            }
        }
        _ => f.conversion = BAD,
    }
    (f, s + 1)
}

/// `long2str` (and `longlong2str`, the same on 64 bits): the number's characters as the
/// conversion asks, or none past the buffer's room.
fn long2str(num: i64, wide: bool, f: &mut Format) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    let mut unsigned = if wide {
        num as u64
    } else {
        num as i32 as u32 as u64
    };
    let mut minus = false;
    let mut digits = 0;
    if num == 0 && f.precision == 0 && !(f.alternate_form && f.conversion == b'o') {
        return Some(out);
    }
    let base: u64 = match f.conversion {
        b'd' | b'i' => {
            let negative = if wide { num < 0 } else { (num as i32) < 0 };
            if negative {
                unsigned = if wide {
                    unsigned.wrapping_neg()
                } else {
                    u64::from((unsigned as u32).wrapping_neg())
                };
                minus = true;
            }
            10
        }
        b'o' => {
            f.sign = Sign::OnlyMinus;
            8
        }
        b'u' => {
            f.sign = Sign::OnlyMinus;
            10
        }
        _ => {
            f.sign = Sign::OnlyMinus;
            16
        }
    };
    loop {
        let n = (unsigned % base) as u8;
        unsigned /= base;
        out.push(if n < 10 {
            b'0' + n
        } else if f.conversion == b'x' {
            b'a' + n - 10
        } else {
            b'A' + n - 10
        });
        digits += 1;
        if unsigned == 0 {
            break;
        }
    }
    if base == 8 && f.alternate_form && out.last() != Some(&b'0') {
        out.push(b'0');
        digits += 1;
    }
    if f.justify == Justify::ZeroFill {
        f.precision = f.field_width;
        if minus || f.sign != Sign::OnlyMinus {
            f.precision -= 1;
        }
        if base == 16 && f.alternate_form {
            f.precision -= 2;
        }
    }
    // `buff - p + precision > 509`: `p` past the terminator and the digits.
    if 1 + digits + f.precision > 509 {
        return None;
    }
    while digits < f.precision {
        out.push(b'0');
        digits += 1;
    }
    if base == 16 && f.alternate_form {
        out.push(f.conversion);
        out.push(b'0');
    }
    if minus {
        out.push(b'-');
    } else if f.sign == Sign::Always {
        out.push(b'+');
    } else if f.sign == Sign::Space {
        out.push(b' ');
    }
    out.reverse();
    Some(out)
}

/// `decimal`: a sign, a decimal exponent and up to 36 significant digits.
#[derive(Clone, Copy)]
struct Decimal {
    sign: bool,
    exp: i32,
    len: usize,
    text: [u8; 36],
}

/// `bit_values` and `digit_values`.
const BIT_VALUES: [f64; 9] = [1e1, 1e2, 1e4, 1e8, 1e16, 1e32, 1e64, 1e128, 1e256];
const DIGIT_VALUES: [f64; 8] = [1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8];

/// `frexp`'s exponent (MSL's, fdlibm's): `x` as a fraction in [0.5, 1) times two to it.
#[doc(alias = "frexp")]
fn frexp_exp(x: f64) -> i32 {
    let bits = x.to_bits();
    let hx = (bits >> 32) as u32;
    let lx = bits as u32;
    let mut ix = hx & 0x7FFF_FFFF;
    if ix >= 0x7FF0_0000 || (ix | lx) == 0 {
        return 0;
    }
    let mut e = 0;
    if ix < 0x0010_0000 {
        // A subnormal, scaled by 2^54 first.
        let y = fp::fmul(x, 18_014_398_509_481_984.0);
        ix = ((y.to_bits() >> 32) as u32) & 0x7FFF_FFFF;
        e = -54;
    }
    e + (ix >> 20) as i32 - 1022
}

/// `__num2dec`: `x`'s first `digits` significant decimal digits (at most 16 computed, the
/// rest zeros to 36), truncated, and its exponent.
#[doc(alias = "__num2dec")]
fn num2dec(digits_wanted: i32, mut x: f64) -> Decimal {
    let mut d = Decimal {
        sign: false,
        exp: 0,
        len: 1,
        text: [0; 36],
    };
    let mut digits = digits_wanted.min(16);
    if x == 0.0 {
        d.text[0] = b'0';
        return d;
    }
    let hi = (x.to_bits() >> 32) as u32;
    if hi & 0x7FF0_0000 == 0x7FF0_0000 {
        d.text[0] = if x.is_nan() { b'N' } else { b'I' };
        return d;
    }
    d.len = 0;
    if x < 0.0 {
        x = fp::fneg(x);
        d.sign = true;
    }
    let e2 = frexp_exp(x);
    let mut n = e2.wrapping_mul(301_029) / 1_000_000;
    let mut exp = n;
    let mut v = 0;
    if n < 0 {
        n = -n;
        while n != 0 {
            if n & 1 != 0 {
                x = fp::fmul(x, BIT_VALUES[v]);
            }
            n >>= 1;
            v += 1;
        }
    } else if n > 0 {
        let mut scale = 1.0;
        while n != 0 {
            if n & 1 != 0 {
                scale = fp::fmul(scale, BIT_VALUES[v]);
            }
            n >>= 1;
            v += 1;
        }
        x = fp::fdiv(x, scale);
    }
    while x >= 1.0 {
        x = fp::fmul(x, 0.1);
        exp += 1;
    }
    while x < 0.1 {
        x = fp::fmul(x, 10.0);
        exp -= 1;
    }
    let mut p = 0;
    while digits != 0 {
        let k = digits.min(8);
        d.len += k as usize;
        digits -= k;
        exp -= k;
        p += k as usize;
        x = fp::fmul(x, DIGIT_VALUES[k as usize - 1]);
        let mut r = fp::fctiwz(x);
        x = fp::fsub(x, f64::from(r));
        for i in 0..k as usize {
            d.text[p - 1 - i] = b'0' + (r % 10) as u8;
            r /= 10;
        }
    }
    let digits = digits_wanted.min(36);
    let pad = digits - d.len as i32;
    if pad > 0 {
        for i in 0..pad as usize {
            d.text[p + i] = b'0';
        }
        exp -= pad;
        d.len += pad as usize;
    }
    d.exp = exp;
    d
}

/// `round_decimal`: the digits rounded to `new_length`, half to even.
fn round_decimal(d: &mut Decimal, new_length: i32) {
    let zero = |d: &mut Decimal| {
        d.sign = false;
        d.exp = 0;
        d.len = 1;
        d.text[0] = b'0';
    };
    if new_length < 0 {
        zero(d);
        return;
    }
    let mut new_length = new_length as usize;
    if new_length >= d.len {
        return;
    }
    let mut p = new_length;
    let c = d.text[p] as i32 - i32::from(b'0');
    let mut carry = if c == 5 {
        let mut q = d.len;
        loop {
            q -= 1;
            if !(q > p && d.text[q] == b'0') {
                break;
            }
        }
        if q == p {
            // `p[-1]`: before the first digit, the length byte.
            i32::from(if p == 0 { d.len as u8 } else { d.text[p - 1] } & 1)
        } else {
            1
        }
    } else {
        i32::from(c > 5)
    };
    while new_length != 0 {
        p -= 1;
        let c = d.text[p] as i32 - i32::from(b'0') + carry;
        carry = i32::from(c > 9);
        if carry != 0 || c == 0 {
            new_length -= 1;
        } else {
            d.text[p] = c as u8 + b'0';
            break;
        }
    }
    if carry != 0 {
        d.exp += 1;
        d.len = 1;
        d.text[0] = b'1';
        return;
    } else if new_length == 0 {
        zero(d);
        return;
    }
    d.len = new_length;
}

/// `float2str`: the number's characters as the conversion (`f`, `e`, `g`) asks, or none past
/// the buffer's room.
fn float2str(num: f64, f: &mut Format) -> Option<Vec<u8>> {
    if f.precision > 509 {
        return None;
    }
    let mut d = num2dec(0x20, num);
    while d.len > 1 && d.text[d.len - 1] == b'0' {
        d.len -= 1;
        d.exp += 1;
    }
    match d.text[0] {
        b'0' => d.exp = 0,
        b'I' => {
            return Some(if num < 0.0 {
                b"-Inf".to_vec()
            } else {
                b"Inf".to_vec()
            });
        }
        b'N' => return Some(b"NaN".to_vec()),
        _ => {}
    }
    d.exp += d.len as i32 - 1;
    // Built backwards from the buffer's end, as the original does.
    let mut out: Vec<u8> = Vec::new();
    let sign = |out: &mut Vec<u8>, d: &Decimal, f: &Format| {
        if d.sign {
            out.push(b'-');
        } else if f.sign == Sign::Always {
            out.push(b'+');
        } else if f.sign == Sign::Space {
            out.push(b' ');
        }
    };
    let mut conversion = f.conversion;
    if conversion == b'g' || conversion == b'G' {
        if d.len as i32 > f.precision {
            round_decimal(&mut d, f.precision);
        }
        if d.exp < -4 || d.exp >= f.precision {
            if f.alternate_form {
                f.precision -= 1;
            } else {
                f.precision = d.len as i32 - 1;
            }
            conversion = if conversion == b'g' { b'e' } else { b'E' };
            f.conversion = conversion;
        } else {
            if f.alternate_form {
                f.precision -= d.exp + 1;
            } else {
                f.precision = (d.len as i32 - (d.exp + 1)).max(0);
            }
            conversion = b'f';
        }
    }
    if conversion == b'e' || conversion == b'E' {
        if d.len as i32 > f.precision + 1 {
            round_decimal(&mut d, f.precision + 1);
        }
        let mut n = d.exp;
        let mut s = b'+';
        if n < 0 {
            n = -n;
            s = b'-';
        }
        let mut digits = 0;
        while n != 0 || digits < 2 {
            out.push((n % 10) as u8 + b'0');
            n /= 10;
            digits += 1;
        }
        out.push(s);
        out.push(conversion);
        if 1 + out.len() as i32 + f.precision > 509 {
            return None;
        }
        if (d.len as i32) < f.precision + 1 {
            let n = (f.precision + 1 - d.len as i32) as usize;
            out.extend(std::iter::repeat_n(b'0', n));
        }
        for i in (1..d.len).rev() {
            out.push(d.text[i]);
        }
        if f.precision != 0 || f.alternate_form {
            out.push(b'.');
        }
        out.push(d.text[0]);
        sign(&mut out, &d, f);
    } else if conversion == b'f' {
        let mut frac = (-d.exp + d.len as i32 - 1).max(0);
        if frac > f.precision {
            let n = d.len as i32 - (frac - f.precision);
            round_decimal(&mut d, n);
            frac = (-d.exp + d.len as i32 - 1).max(0);
        }
        let int = (d.exp + 1).max(0);
        if int + frac > 509 {
            return None;
        }
        let mut q = d.len;
        out.extend(std::iter::repeat_n(
            b'0',
            (f.precision - frac).max(0) as usize,
        ));
        let mut digits = 0;
        while digits < frac && digits < d.len as i32 {
            q -= 1;
            out.push(d.text[q]);
            digits += 1;
        }
        while digits < frac {
            out.push(b'0');
            digits += 1;
        }
        if f.precision != 0 || f.alternate_form {
            out.push(b'.');
        }
        if int != 0 {
            let mut digits = 0;
            while digits < int - d.len as i32 {
                out.push(b'0');
                digits += 1;
            }
            while digits < int {
                q -= 1;
                out.push(d.text[q]);
                digits += 1;
            }
        } else {
            out.push(b'0');
        }
        sign(&mut out, &d, f);
    }
    out.reverse();
    Some(out)
}

/// `vsnprintf` with no limit (`vsprintf`, and `DevText_Printf`'s, which passes -1): what
/// `__pformatter` writes for `format` and `args`, without the terminator.
#[doc(alias = "vsnprintf")]
#[doc(alias = "__pformatter")]
#[doc(alias = "parse_format")]
#[doc(alias = "long2str")]
#[doc(alias = "float2str")]
#[doc(alias = "round_decimal")]
#[doc(alias = "__num2dec")]
#[doc(alias = "__StringWrite")]
pub fn vsprintf(format: &[u8], args: &[Arg]) -> Vec<u8> {
    let fmt = &format[..format.iter().position(|&c| c == 0).unwrap_or(format.len())];
    let mut args = Args {
        list: args,
        next: 0,
    };
    let mut out = Vec::new();
    let mut s = 0;
    while s < fmt.len() {
        let Some(pct) = fmt[s..].iter().position(|&c| c == b'%').map(|i| s + i) else {
            out.extend_from_slice(&fmt[s..]);
            break;
        };
        out.extend_from_slice(&fmt[s..pct]);
        let (mut f, next) = parse_format(fmt, pct, &mut args);
        s = next;
        let body: Option<Vec<u8>> = match f.conversion {
            b'd' | b'i' | b'o' | b'u' | b'x' | b'X' => {
                let wide = f.arg == ArgKind::LongLong;
                let mut n = i64::from(args.int());
                let signed = matches!(f.conversion, b'd' | b'i');
                if f.arg == ArgKind::Short {
                    n = if signed {
                        i64::from(n as i16)
                    } else {
                        i64::from(n as u16)
                    };
                }
                if f.arg == ArgKind::Char {
                    n = if signed {
                        i64::from(n as i8)
                    } else {
                        i64::from(n as u8)
                    };
                }
                assert!(!wide, "a long long argument isn't ported");
                long2str(n, wide, &mut f)
            }
            b'f' | b'e' | b'E' | b'g' | b'G' => {
                let v = args.float();
                float2str(v, &mut f)
            }
            b's' => {
                assert!(
                    f.arg != ArgKind::Wchar,
                    "a wide string argument isn't ported"
                );
                let p = args.str();
                let p = &p[..p.iter().position(|&c| c == 0).unwrap_or(p.len())];
                // A Pascal string (`%#s`, its length first) reads past its terminator: no
                // caller formats one.
                assert!(!f.alternate_form, "a %#s conversion isn't ported");
                Some(if f.precision_specified {
                    p.iter().copied().take(f.precision as usize).collect()
                } else {
                    p.to_vec()
                })
            }
            b'c' => Some(vec![args.int() as u8]),
            b'%' => Some(vec![b'%']),
            b'n' => panic!("a %n conversion isn't ported"),
            _ => None,
        };
        let Some(mut body) = body else {
            // `conversion_error`: the rest of the format as it stands, and the end.
            out.extend_from_slice(&fmt[pct..]);
            return out;
        };
        let mut width = body.len() as i32;
        if f.justify != Justify::Left {
            let fill = if f.justify == Justify::ZeroFill {
                b'0'
            } else {
                b' '
            };
            if (body.first() == Some(&b'+') || body.first() == Some(&b'-')) && fill == b'0' {
                out.push(body.remove(0));
            }
            while width < f.field_width {
                out.push(fill);
                width += 1;
            }
        }
        out.extend_from_slice(&body);
        if f.justify == Justify::Left {
            while width < f.field_width {
                out.push(b' ');
                width += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(f: &str, a: &[Arg]) -> String {
        String::from_utf8(vsprintf(f.as_bytes(), a)).unwrap()
    }

    #[test]
    fn formats_as_msl_does() {
        assert_eq!(
            p(
                "%d %2d %3d|%7d",
                &[Arg::Int(5), Arg::Int(-3), Arg::Int(42), Arg::Int(123)]
            ),
            "5 -3  42|    123"
        );
        assert_eq!(p("%2.2f", &[Arg::Float(1.0)]), "1.00");
        assert_eq!(p("%03.2f", &[Arg::Float(7.5)]), "7.50");
        assert_eq!(p("%03.2f", &[Arg::Float(0.125)]), "0.12");
        assert_eq!(p("%.2f", &[Arg::Float(0.375)]), "0.38");
        assert_eq!(p("%2.2f", &[Arg::Float(-2.005)]), "-2.00");
        assert_eq!(
            p("%s : %d", &[Arg::Str(b"Bonus\0"), Arg::Int(10)]),
            "Bonus : 10"
        );
        assert_eq!(p("%02d%%", &[Arg::Int(7)]), "07%");
        assert_eq!(p("%x", &[Arg::Int(0x1ABC)]), "1abc");
        assert_eq!(p("%f", &[Arg::Float(123.456)]), "123.456000");
        assert_eq!(p("%.2f", &[Arg::Float(99999.999)]), "100000.00");
    }
}
