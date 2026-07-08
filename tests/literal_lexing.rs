//! Integration tests for numeric/list-literal lexer and grammar gaps fixed
//! on this branch:
//!
//! 1. Octal integer literals (`0oNNN`), including the sign-folded radix-8
//!    value conversion.
//! 2. Leading-dot decimal float literals (`.5`, `-.1e-5`, …).
//! 3. A negative number as the first element of a list literal (`[-1, 2]`).
//! 4. The most-negative `i64` literal edge case (`-9223372036854775808`),
//!    which overflows as a positive magnitude but not once negated.
//!
//! Each test also pins down that the corresponding "should fail" TCK
//! boundary case (genuine integer overflow) is *still* rejected, so the fix
//! doesn't overshoot into silently wrapping/truncating out-of-range
//! literals.

use assert2::check;
use decypher::ast::expr::{Expression, Literal, NumberLiteral, UnaryOperator};
use decypher::ast::pattern::{LabelExpression, PatternElement};
use decypher::ast::query::{QueryBody, ReadingClause, SinglePartBody};
use decypher::parse;

fn first_projection_expr(query: &decypher::ast::Query) -> &Expression {
    let QueryBody::SingleQuery(sq) = &query.statements[0] else {
        panic!("expected SingleQuery");
    };
    let decypher::ast::query::SingleQueryKind::SinglePart(spq) = &sq.kind else {
        panic!("expected SinglePart query");
    };
    let SinglePartBody::Return(ret) = &spq.body else {
        panic!("expected Return body");
    };
    &ret.items[0].expression
}

fn first_match_types(query: &decypher::ast::Query) -> LabelExpression {
    let QueryBody::SingleQuery(sq) = &query.statements[0] else {
        panic!("expected SingleQuery");
    };
    let decypher::ast::query::SingleQueryKind::SinglePart(spq) = &sq.kind else {
        panic!("expected SinglePart query");
    };
    let ReadingClause::Match(m) = &spq.reading_clauses[0] else {
        panic!("expected Match reading clause");
    };
    let PatternElement::Path { chains, .. } = &m.pattern.parts[0].anonymous.element else {
        panic!("expected Path pattern element");
    };
    chains[0]
        .relationship
        .detail
        .as_ref()
        .expect("expected relationship detail")
        .types
        .clone()
        .expect("expected relationship types")
}

// ============================================================
// 1. Octal integer literals
// ============================================================

/// `0o52` is octal for 42.
///
/// Unit: `parse()` / AST `NumberLiteral::Integer`
/// Precondition: `RETURN 0o52;`.
/// Expectation: the literal value is exactly `42`.
#[test]
fn test_octal_literal_value() {
    let query = parse("RETURN 0o52;").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::Number(NumberLiteral::Integer(v))) => {
            check!(*v == 42);
        }
        other => panic!("expected Integer literal, got {other:?}"),
    }
}

/// A negative octal literal parses to its correct magnitude, `372036854`,
/// wrapped in a unary negation (the ordinary path: the octal fix only needs
/// `parse_integer` to understand the `0o` prefix, since this magnitude
/// comfortably fits as a positive `i64` before negation).
///
/// Unit: `parse()` / AST `UnaryOp` + `NumberLiteral::Integer`
/// Precondition: `RETURN -0o2613152366;`.
/// Expectation: `UnaryOp::Negate` wrapping `Integer(372036854)` (i.e. the
/// value is `-372036854` once evaluated).
#[test]
fn test_negative_octal_literal_value() {
    let query = parse("RETURN -0o2613152366;").unwrap();
    match first_projection_expr(&query) {
        Expression::UnaryOp {
            op: UnaryOperator::Negate,
            operand,
            ..
        } => match operand.as_ref() {
            Expression::Literal(Literal::Number(NumberLiteral::Integer(v))) => {
                check!(*v == 372_036_854);
            }
            other => panic!("expected Integer operand, got {other:?}"),
        },
        other => panic!("expected UnaryOp Negate, got {other:?}"),
    }
}

/// The largest representable octal integer, `0o777777777777777777777`
/// (21 sevens = 2^63 - 1), is exactly `i64::MAX`.
///
/// Unit: `parse()` / AST `NumberLiteral::Integer`
/// Precondition: `RETURN 0o777777777777777777777;`.
/// Expectation: the literal value is exactly `i64::MAX`.
#[test]
fn test_largest_octal_literal_is_i64_max() {
    let query = parse("RETURN 0o777777777777777777777;").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::Number(NumberLiteral::Integer(v))) => {
            check!(*v == i64::MAX);
        }
        other => panic!("expected Integer literal, got {other:?}"),
    }
}

/// An octal integer literal whose positive magnitude exceeds `i64::MAX`
/// must still be rejected (it is not silently wrapped or truncated).
///
/// Unit: `parse()`
/// Precondition: `RETURN 0o1000000000000000000000;` (magnitude 2^63).
/// Expectation: `parse()` returns `Err`.
#[test]
fn test_octal_overflow_still_rejected() {
    let result = parse("RETURN 0o1000000000000000000000;");
    check!(result.is_err());
}

// ============================================================
// 2. Leading-dot float literals
// ============================================================

/// `.5` (no digits before the decimal point) is a valid float literal.
///
/// Unit: `parse()` / AST `NumberLiteral::Float`
/// Precondition: `RETURN .5;`.
/// Expectation: the literal value is exactly `0.5`.
#[test]
fn test_leading_dot_float_literal_value() {
    let query = parse("RETURN .5;").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::Number(NumberLiteral::Float(v))) => {
            check!(*v == 0.5);
        }
        other => panic!("expected Float literal, got {other:?}"),
    }
}

/// A negative, exponent-bearing leading-dot float: `-.1e-5` == `-0.000001`.
///
/// Unit: `parse()` / AST `UnaryOp` + `NumberLiteral::Float`
/// Precondition: `RETURN -.1e-5;`.
/// Expectation: a `UnaryOp::Negate` wrapping a `Float(1e-6)` operand.
#[test]
fn test_negative_leading_dot_exponent_float() {
    let query = parse("RETURN -.1e-5;").unwrap();
    match first_projection_expr(&query) {
        Expression::UnaryOp {
            op: UnaryOperator::Negate,
            operand,
            ..
        } => match operand.as_ref() {
            Expression::Literal(Literal::Number(NumberLiteral::Float(v))) => {
                check!((*v - 1e-6).abs() < 1e-18);
            }
            other => panic!("expected Float operand, got {other:?}"),
        },
        other => panic!("expected UnaryOp Negate, got {other:?}"),
    }
}

/// A leading-dot float must not be confused with property access: `n.age`
/// still parses as a `PropertyLookup`, not a malformed number.
///
/// Unit: `parse()` / AST `Expression::PropertyLookup`
/// Precondition: `MATCH (n) RETURN n.age;`.
/// Expectation: parses successfully to a `PropertyLookup` expression.
#[test]
fn test_property_lookup_not_confused_with_leading_dot_float() {
    let query = parse("MATCH (n) RETURN n.age;").unwrap();
    match first_projection_expr(&query) {
        Expression::PropertyLookup { .. } => {}
        other => panic!("expected PropertyLookup, got {other:?}"),
    }
}

/// The `..` range-slice operator must not be confused with a leading-dot
/// float: `list[1..3]` still parses as a `ListSlice`.
///
/// Unit: `parse()`
/// Precondition: `RETURN range(1, 5)[1..3];`.
/// Expectation: `parse()` returns `Ok`.
#[test]
fn test_range_slice_not_confused_with_leading_dot_float() {
    let result = parse("RETURN range(1, 5)[1..3];");
    check!(result.is_ok(), "{:?}", result.err());
}

// ============================================================
// 3. Negative number as the first element of a list literal
// ============================================================

/// `[-1, 2]` is a two-element list literal, not a mis-parsed pattern
/// comprehension.
///
/// Unit: `parse()` / AST `Literal::List`
/// Precondition: `RETURN [-1, 2];`.
/// Expectation: two elements: `UnaryOp::Negate(Integer(1))`, `Integer(2)`.
#[test]
fn test_negative_first_list_element() {
    let query = parse("RETURN [-1, 2];").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::List(list)) => {
            check!(list.elements.len() == 2);
            match &list.elements[0] {
                Expression::UnaryOp {
                    op: UnaryOperator::Negate,
                    operand,
                    ..
                } => match operand.as_ref() {
                    Expression::Literal(Literal::Number(NumberLiteral::Integer(v))) => {
                        check!(*v == 1);
                    }
                    other => panic!("expected Integer operand, got {other:?}"),
                },
                other => panic!("expected UnaryOp Negate, got {other:?}"),
            }
            match &list.elements[1] {
                Expression::Literal(Literal::Number(NumberLiteral::Integer(v))) => {
                    check!(*v == 2);
                }
                other => panic!("expected Integer literal, got {other:?}"),
            }
        }
        other => panic!("expected List literal, got {other:?}"),
    }
}

/// An anonymous pattern comprehension with a leading dash must still parse
/// as a pattern (not get misclassified as a negative-number list) — the
/// disambiguation added for `[-1, 2]` must not regress this sibling form.
///
/// Unit: `parse()`
/// Precondition: `MATCH (a) RETURN [(a)-->(b) | b.name];`.
/// Expectation: `parse()` returns `Ok`.
#[test]
fn test_pattern_comprehension_still_parses() {
    let result = parse("MATCH (a) RETURN [(a)-->(b) | b.name];");
    check!(result.is_ok(), "{:?}", result.err());
}

// ============================================================
// 4. i64::MIN literal edge case
// ============================================================

/// The most-negative `i64` value, `-9223372036854775808`, parses exactly —
/// its positive magnitude (2^63) alone overflows `i64::MAX`, so the sign
/// must be folded in before range-checking.
///
/// Unit: `parse()` / AST `NumberLiteral::Integer`
/// Precondition: `RETURN -9223372036854775808;`.
/// Expectation: the literal value is exactly `i64::MIN`.
#[test]
fn test_i64_min_decimal_literal() {
    let query = parse("RETURN -9223372036854775808;").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::Number(NumberLiteral::Integer(v))) => {
            check!(*v == i64::MIN);
        }
        other => panic!("expected Integer literal, got {other:?}"),
    }
}

/// The hexadecimal form of the same edge case: `-0x8000000000000000`.
///
/// Unit: `parse()` / AST `NumberLiteral::Integer`
/// Precondition: `RETURN -0x8000000000000000;`.
/// Expectation: the literal value is exactly `i64::MIN`.
#[test]
fn test_i64_min_hex_literal() {
    let query = parse("RETURN -0x8000000000000000;").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::Number(NumberLiteral::Integer(v))) => {
            check!(*v == i64::MIN);
        }
        other => panic!("expected Integer literal, got {other:?}"),
    }
}

/// One past `i64::MIN` must still be rejected as an overflow, confirming
/// the sign-folding fix doesn't overshoot into accepting out-of-range
/// literals.
///
/// Unit: `parse()`
/// Precondition: `RETURN -9223372036854775809;`.
/// Expectation: `parse()` returns `Err`.
#[test]
fn test_below_i64_min_still_rejected() {
    let result = parse("RETURN -9223372036854775809;");
    check!(result.is_err());
}

/// One past `i64::MAX` (positive, no sign) must still be rejected.
///
/// Unit: `parse()`
/// Precondition: `RETURN 9223372036854775808;`.
/// Expectation: `parse()` returns `Err`.
#[test]
fn test_above_i64_max_still_rejected() {
    let result = parse("RETURN 9223372036854775808;");
    check!(result.is_err());
}

// ============================================================
// 5. Colon-repeated relationship-type alternation
// ============================================================

/// The non-repeated form `[:T|U]` must keep working unchanged.
///
/// Unit: `parse()` / AST `LabelExpression::Or`
/// Precondition: `MATCH (a)-[:T|U]->(b) RETURN b;`.
/// Expectation: an `Or` of `Static("T")` and `Static("U")`.
#[test]
fn test_non_repeated_relationship_type_alternation_still_works() {
    let query = parse("MATCH (a)-[:T|U]->(b) RETURN b;").unwrap();
    match first_match_types(&query) {
        LabelExpression::Or { lhs, rhs, .. } => {
            match *lhs {
                LabelExpression::Static(name) => {
                    check!(name.name == "T");
                }
                other => panic!("expected Static lhs, got {other:?}"),
            }
            match *rhs {
                LabelExpression::Static(name) => {
                    check!(name.name == "U");
                }
                other => panic!("expected Static rhs, got {other:?}"),
            }
        }
        other => panic!("expected Or label expression, got {other:?}"),
    }
}
