use crate::error::{CypherError, ErrorKind, Span};

/// Decode escape sequences from a string literal's content (already stripped of quotes).
pub fn decode_string_content(content: &str, span: Span) -> (String, Option<CypherError>) {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') => result.push('\\'),
                Some('\'') => result.push('\''),
                Some('"') => result.push('"'),
                Some('b') | Some('B') => result.push('\u{0008}'),
                Some('f') | Some('F') => result.push('\u{000C}'),
                Some('n') | Some('N') => result.push('\n'),
                Some('r') | Some('R') => result.push('\r'),
                Some('t') | Some('T') => result.push('\t'),
                Some('u') | Some('U') => {
                    let mut hex = String::new();
                    let mut count = 0;
                    while count < 8 && chars.peek().is_some() {
                        let next = *chars.peek().unwrap();
                        if next.is_ascii_hexdigit() {
                            hex.push(chars.next().unwrap());
                            count += 1;
                        } else {
                            break;
                        }
                    }
                    if count == 4 || count == 8 {
                        if let Ok(codepoint) = u32::from_str_radix(&hex, 16) {
                            if let Some(c) = char::from_u32(codepoint) {
                                result.push(c);
                            } else {
                                let err_sp = Span::new(span.start, span.end);
                                return (
                                    result,
                                    Some(CypherError {
                                        kind: ErrorKind::InvalidEscape {
                                            sequence: format!("\\u{}", hex),
                                        },
                                        span: err_sp,
                                        source_label: None,
                                        notes: Vec::new(),
                                        source: None,
                                    }),
                                );
                            }
                        }
                    } else {
                        let err_sp = Span::new(span.start, span.end);
                        return (
                            result,
                            Some(CypherError {
                                kind: ErrorKind::InvalidEscape {
                                    sequence: format!("\\u{}", hex),
                                },
                                span: err_sp,
                                source_label: None,
                                notes: Vec::new(),
                                source: None,
                            }),
                        );
                    }
                }
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => {
                    result.push('\\');
                }
            }
        } else {
            result.push(ch);
        }
    }
    (result, None)
}

/// Parse an integer from a string, handling `0x`/`0X` hex, `0o`/`0O` octal,
/// legacy `0`-prefixed octal, and plain decimal — each optionally preceded
/// by a single leading `-` sign.
///
/// The sign is folded in *before* range-checking (via an `i128`
/// intermediate) rather than parsed as a positive magnitude and negated
/// afterwards. This matters for the most-negative `i64` value: its
/// magnitude, `9223372036854775808`, does not fit in an `i64` even though
/// the final negated value (`i64::MIN`) does.
pub fn parse_integer(s: &str) -> Option<i64> {
    let s = s.trim();
    let (negative, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let magnitude: i128 = if digits.starts_with("0x") || digits.starts_with("0X") {
        i128::from_str_radix(&digits[2..], 16).ok()?
    } else if digits.starts_with("0o") || digits.starts_with("0O") {
        i128::from_str_radix(&digits[2..], 8).ok()?
    } else if digits.starts_with('0')
        && digits.len() > 1
        && digits.chars().all(|c| c.is_ascii_digit())
    {
        i128::from_str_radix(digits, 8).ok()?
    } else {
        digits.parse::<i128>().ok()?
    };
    let signed = if negative { -magnitude } else { magnitude };
    i64::try_from(signed).ok()
}

/// Parse a floating-point number from a string.
pub fn parse_double(s: &str) -> Option<f64> {
    let val = s.trim().parse::<f64>().ok()?;
    if val.is_nan() || val.is_infinite() {
        None
    } else {
        Some(val)
    }
}
