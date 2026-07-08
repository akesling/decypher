//! Integration tests for contextual (non-reserved) keyword handling.
//!
//! openCypher distinguishes *reserved* words (`MATCH`, `RETURN`, `WHERE`,
//! `WITH`, `CREATE`, …), which can never be used as identifiers, from a
//! larger set of *non-reserved* keywords (`NODE`, `PROPERTY`, `ROWS`,
//! `TYPES`, …) that introduce specific constructs elsewhere in the grammar
//! but remain ordinary identifiers everywhere else. These tests verify:
//!
//! - non-reserved keywords parse as a bare variable/expression reference,
//!   not just in binding position (`AS rows`), which already worked;
//! - `null`, `true`, and `false` are accepted as unquoted map-literal keys,
//!   per openCypher's `SchemaName ::= SymbolicName | reservedWord`
//!   production;
//! - genuinely reserved words are *still* rejected as bare identifiers, so
//!   the fix does not over-loosen clause parsing.

use assert2::check;
use decypher::ast::expr::Expression;
use decypher::ast::query::{QueryBody, SinglePartBody};
use decypher::parse;

/// Extracts the first projection expression from a query's terminating
/// `RETURN` clause, whether the query is single-part (`CREATE (n) RETURN
/// n`) or multi-part (`WITH … RETURN …`), and whether the `RETURN` sits
/// directly in the body or trails a run of updating clauses.
fn first_projection_expr(query: &decypher::ast::Query) -> &Expression {
    let QueryBody::SingleQuery(sq) = &query.statements[0] else {
        panic!("expected SingleQuery");
    };
    let spq = match &sq.kind {
        decypher::ast::query::SingleQueryKind::SinglePart(spq) => spq,
        decypher::ast::query::SingleQueryKind::MultiPart(mp) => &mp.final_part,
    };
    let ret = match &spq.body {
        SinglePartBody::Return(ret) => ret,
        SinglePartBody::Updating {
            return_clause: Some(ret),
            ..
        } => ret,
        other => panic!("expected a RETURN clause, got {other:?}"),
    };
    &ret.items[0].expression
}

// ── Group 1: non-reserved keywords as bare expression references ───────

/// `rows` is a non-reserved keyword (used in `IN TRANSACTIONS OF n ROWS`)
/// but must still work as an ordinary bound variable reference.
///
/// Unit: `parse()` / AST `Expression::Variable`
/// Precondition: `WITH 1 AS rows RETURN rows`.
/// Expectation: parses, and the `RETURN` item is `Variable("rows")`.
#[test]
fn test_rows_as_bare_identifier() {
    let query = parse("WITH 1 AS rows RETURN rows").unwrap();
    match first_projection_expr(&query) {
        Expression::Variable(v) => {
            check!(v.name.name == "rows");
        }
        other => panic!("expected Variable, got {other:?}"),
    }
}

/// `types` is a non-reserved keyword but must work as a bare variable
/// reference, including as a function argument.
///
/// Unit: `parse()` / AST `Expression::FunctionCall`
/// Precondition: `RETURN size(types)`.
/// Expectation: parses, and the sole argument is `Variable("types")`.
#[test]
fn test_types_as_bare_identifier_in_function_call() {
    let query = parse("RETURN size(types)").unwrap();
    match first_projection_expr(&query) {
        Expression::FunctionCall(fi) => {
            check!(fi.arguments.len() == 1);
            match &fi.arguments[0] {
                Expression::Variable(v) => {
                    check!(v.name.name == "types");
                }
                other => panic!("expected Variable argument, got {other:?}"),
            }
        }
        other => panic!("expected FunctionCall, got {other:?}"),
    }
}

/// `property` is a non-reserved keyword but must work as a bare variable
/// reference in a `WHERE` predicate.
///
/// Unit: `parse()`
/// Precondition: `MATCH (a:Begin) WITH a.num AS property MATCH (b:End)
/// WHERE property = b.num RETURN b`.
/// Expectation: parses successfully.
#[test]
fn test_property_as_bare_identifier_in_where() {
    let result = parse(
        "MATCH (a:Begin) WITH a.num AS property MATCH (b:End) WHERE property = b.num RETURN b",
    );
    check!(result.is_ok());
}

/// `node` is a non-reserved keyword (used in `NODE KEY` constraints) but
/// must work as a bare variable reference, including as a function
/// argument.
///
/// Unit: `parse()` / AST `Expression::FunctionCall`
/// Precondition: `CREATE (node) RETURN labels(node)`.
/// Expectation: parses, and `labels(...)`'s sole argument is
/// `Variable("node")`.
#[test]
fn test_node_as_bare_identifier_in_function_call() {
    let query = parse("CREATE (node) RETURN labels(node)").unwrap();
    match first_projection_expr(&query) {
        Expression::FunctionCall(fi) => {
            check!(fi.arguments.len() == 1);
            match &fi.arguments[0] {
                Expression::Variable(v) => {
                    check!(v.name.name == "node");
                }
                other => panic!("expected Variable argument, got {other:?}"),
            }
        }
        other => panic!("expected FunctionCall, got {other:?}"),
    }
}

/// A node variable named `node` may also carry labels and a property map —
/// exercising the same contextual-keyword path used for `CREATE`'s node
/// pattern parser.
///
/// Unit: `parse()`
/// Precondition: `CREATE (node:Foo:Bar {name: 'Mattias'}) RETURN
/// labels(node)`.
/// Expectation: parses successfully.
#[test]
fn test_node_as_identifier_with_labels_and_properties() {
    let result = parse("CREATE (node:Foo:Bar {name: 'Mattias'}) RETURN labels(node)");
    check!(result.is_ok());
}

// ── Group 2: null/true/false as unquoted map-literal keys ───────────────

/// `null` is a reserved word but openCypher's `SchemaName` production
/// (which backs `PropertyKeyName`) explicitly allows `reservedWord`, so it
/// must be usable as an unquoted map-literal key.
///
/// Unit: `parse()` / AST `Literal::Map`
/// Precondition: `RETURN {null: 1} AS m`.
/// Expectation: parses, with one entry whose key is `"null"`.
#[test]
fn test_null_as_map_key() {
    use decypher::ast::expr::Literal;

    let query = parse("RETURN {null: 1} AS m").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::Map(map)) => {
            check!(map.entries.len() == 1);
            check!(map.entries[0].0.name.name == "null");
        }
        other => panic!("expected Literal::Map, got {other:?}"),
    }
}

/// `true` and `false` must likewise be usable as unquoted map-literal keys.
///
/// Unit: `parse()` / AST `Literal::Map`
/// Precondition: `RETURN {true: 1, false: 2} AS m`.
/// Expectation: parses, with two entries keyed `"true"` and `"false"`.
#[test]
fn test_true_false_as_map_keys() {
    use decypher::ast::expr::Literal;

    let query = parse("RETURN {true: 1, false: 2} AS m").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::Map(map)) => {
            check!(map.entries.len() == 2);
            check!(map.entries[0].0.name.name == "true");
            check!(map.entries[1].0.name.name == "false");
        }
        other => panic!("expected Literal::Map, got {other:?}"),
    }
}

/// Both a lowercase `null` and an uppercase `NULL` key can coexist in the
/// same map literal (the TCK's actual repro), and the resolved variable
/// referencing that map still parses/round-trips through property lookup
/// and index-lookup syntax.
///
/// Unit: `parse()`
/// Precondition: `WITH {null: 'Mats', NULL: 'Pontus'} AS map RETURN
/// map.\`null\` AS result`.
/// Expectation: parses successfully.
#[test]
fn test_null_and_uppercase_null_as_map_keys() {
    let result = parse("WITH {null: 'Mats', NULL: 'Pontus'} AS map RETURN map.`null` AS result");
    check!(result.is_ok());
}

// ── Negative: genuinely reserved words stay unusable as identifiers ────

/// `MATCH` is a genuinely reserved word. Unlike `rows`/`types`/`property`/
/// `node`, it must remain rejected as a bare expression reference — this
/// guards against the contextual-keyword fix over-loosening clause parsing.
///
/// Unit: `parse()`
/// Precondition: `RETURN MATCH`.
/// Expectation: returns `Err`.
#[test]
fn test_match_still_rejected_as_bare_identifier() {
    let result = parse("RETURN MATCH");
    check!(result.is_err());
}

/// Even when a reserved word is used as an `AS` alias target, referencing
/// it back as a bare expression must still fail — reserved words are not
/// promoted to usable identifiers just because they parsed in binding
/// position.
///
/// Unit: `parse()`
/// Precondition: `WITH 1 AS MATCH RETURN MATCH`.
/// Expectation: returns `Err`.
#[test]
fn test_with_as_match_still_rejected() {
    let result = parse("WITH 1 AS MATCH RETURN MATCH");
    check!(result.is_err());
}

/// `WHERE`, `WITH`, and `CREATE` are likewise genuinely reserved and must
/// stay rejected as bare expression references.
///
/// Unit: `parse()`
/// Precondition: `RETURN WHERE`, `RETURN WITH`, `RETURN CREATE`.
/// Expectation: all three return `Err`.
#[test]
fn test_other_reserved_words_still_rejected_as_bare_identifiers() {
    check!(parse("RETURN WHERE").is_err());
    check!(parse("RETURN WITH").is_err());
    check!(parse("RETURN CREATE").is_err());
}
