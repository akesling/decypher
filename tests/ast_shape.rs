//! Integration tests that verify the structural shape of the parsed AST.
//!
//! These tests exercise the typed AST node hierarchy by inspecting fields
//! such as `statements`, `reading_clauses`, `body`, etc. to confirm that
//! the parser constructs the correct AST variants for various query shapes.

use assert2::check;
use decypher::ast::query::{QueryBody, SinglePartBody};
use decypher::parse;

/// A `MATCH … RETURN n` query produces a `SinglePartBody::Return` with one
/// projection item.
///
/// Unit: `parse()` / AST `SinglePartBody`
/// Precondition: `MATCH (n) RETURN n;` — single MATCH and a RETURN with one item.
/// Expectation: AST has `SinglePartBody::Return` with `items.len() == 1`.
#[test]
fn test_match_return_has_return_body() {
    let query = parse("MATCH (n) RETURN n;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            decypher::ast::query::SingleQueryKind::SinglePart(spq) => match &spq.body {
                SinglePartBody::Return(ret) => {
                    check!(ret.items.len() == 1);
                }
                _ => panic!("expected Return body"),
            },
            _ => panic!("expected SinglePart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// A `MATCH … CREATE …` query produces a `SinglePartBody::Updating` with one
/// updating clause and no RETURN.
///
/// Unit: `parse()` / AST `SinglePartBody`
/// Precondition: `MATCH (a) CREATE (a)-[:KNOWS]->(b);`.
/// Expectation: `updating.len() == 1` and `return_clause.is_none()`.
#[test]
fn test_match_create_has_updating_body() {
    let query = parse("MATCH (a) CREATE (a)-[:KNOWS]->(b);").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            decypher::ast::query::SingleQueryKind::SinglePart(spq) => match &spq.body {
                SinglePartBody::Updating {
                    updating,
                    return_clause,
                } => {
                    check!(updating.len() == 1);
                    check!(return_clause.is_none());
                }
                _ => panic!("expected Updating body"),
            },
            _ => panic!("expected SinglePart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// An `OPTIONAL MATCH` clause has `optional == true` on the `Match` AST node.
///
/// Unit: `parse()` / AST `Match::optional`
/// Precondition: `OPTIONAL MATCH (n) RETURN n;`.
/// Expectation: `m.optional == true`.
#[test]
fn test_optional_match_flag() {
    let query = parse("OPTIONAL MATCH (n) RETURN n;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            decypher::ast::query::SingleQueryKind::SinglePart(spq) => {
                match &spq.reading_clauses[0] {
                    decypher::ast::query::ReadingClause::Match(m) => {
                        check!(m.optional);
                    }
                    _ => panic!("expected Match clause"),
                }
            }
            _ => panic!("expected SinglePart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// An `UNWIND … AS x` clause stores the binding variable name `"x"`.
///
/// Unit: `parse()` / AST `Unwind::variable`
/// Precondition: `UNWIND [1, 2, 3] AS x RETURN x;`.
/// Expectation: `u.variable.name.name == "x"`.
#[test]
fn test_unwind_has_expression_and_variable() {
    let query = parse("UNWIND [1, 2, 3] AS x RETURN x;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            decypher::ast::query::SingleQueryKind::SinglePart(spq) => {
                match &spq.reading_clauses[0] {
                    decypher::ast::query::ReadingClause::Unwind(u) => {
                        check!(u.variable.name.name == "x");
                    }
                    _ => panic!("expected Unwind clause"),
                }
            }
            _ => panic!("expected SinglePart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// A node pattern `(n:Person)` has a bound variable `"n"` and one label.
///
/// Unit: `parse()` / AST `NodePattern`
/// Precondition: `MATCH (n:Person) RETURN n;`.
/// Expectation: The start node of the first path has `variable.name == "n"` and
///   `labels.len() == 1`.
#[test]
fn test_pattern_has_node() {
    let query = parse("MATCH (n:Person) RETURN n;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => {
            match &sq.kind {
                decypher::ast::query::SingleQueryKind::SinglePart(spq) => {
                    match &spq.reading_clauses[0] {
                        decypher::ast::query::ReadingClause::Match(m) => {
                            check!(m.pattern.parts.len() == 1);
                            let part = &m.pattern.parts[0];
                            // The node variable is inside the anonymous pattern part
                            match &part.anonymous.element {
                                decypher::ast::pattern::PatternElement::Path { start, .. } => {
                                    check!(start.variable.is_some());
                                    check!(start.variable.as_ref().unwrap().name.name == "n");
                                    check!(start.labels.len() == 1);
                                }
                                _ => panic!("expected Path pattern element"),
                            }
                        }
                        _ => panic!("expected Match clause"),
                    }
                }
                _ => panic!("expected SinglePart query"),
            }
        }
        _ => panic!("expected SingleQuery"),
    }
}

/// A `UNION` query is represented in the parsed `Query` statement list.
///
/// Unit: `parse()` / AST `RegularQuery::unions`
/// Precondition: Two MATCH/RETURN branches joined by `UNION`.
/// Expectation: `query.statements.len() >= 1`.
#[test]
fn test_union_has_two_queries() {
    let query =
        parse("MATCH (n:Person) RETURN n.name UNION MATCH (m:Movie) RETURN m.title;").unwrap();
    check!(query.statements.len() >= 1);
    // The UNION creates a RegularQuery with unions
    // Our current structure stores it in RegularQuery.unions
}

/// A parsed query's top-level span is a non-empty range.
///
/// Unit: `parse()` / AST `Query::span`
/// Precondition: `MATCH (n) RETURN n;` — non-empty input.
/// Expectation: `query.span.start < query.span.end`.
#[test]
fn test_span_is_nonzero() {
    let query = parse("MATCH (n) RETURN n;").unwrap();
    check!(query.span.start < query.span.end);
}

/// `range(start, end)` in expression position parses as an ordinary function
/// invocation — not an error, and not a bare variable followed by a stray
/// parenthesis. `range` is a contextual keyword (reserved only in the schema
/// `CREATE RANGE INDEX …` position), so as an expression it is the standard
/// openCypher list function.
///
/// Unit: `parse()` / AST `Expression::FunctionCall`
/// Precondition: `RETURN range(0, 3) AS r;`.
/// Expectation: the projection expression is a `FunctionCall` named `range`
/// with two arguments.
#[test]
fn test_range_is_a_function_invocation() {
    use decypher::ast::expr::Expression;

    let query = parse("RETURN range(0, 3) AS r;").unwrap();
    let QueryBody::SingleQuery(sq) = &query.statements[0] else {
        panic!("expected SingleQuery");
    };
    let decypher::ast::query::SingleQueryKind::SinglePart(spq) = &sq.kind else {
        panic!("expected SinglePart query");
    };
    let SinglePartBody::Return(ret) = &spq.body else {
        panic!("expected Return body");
    };
    match &ret.items[0].expression {
        Expression::FunctionCall(fi) => {
            check!(fi.name.len() == 1);
            check!(fi.name[0].name == "range");
            check!(fi.arguments.len() == 2);
        }
        other => panic!("expected a FunctionCall, got {other:?}"),
    }
}

/// A three-argument `range(start, end, step)` also parses (the optional step is
/// just another ordinary argument).
///
/// Unit: `parse()` / AST `Expression::FunctionCall`
/// Precondition: `RETURN range(0, 10, 2) AS r;`.
/// Expectation: a `FunctionCall` named `range` with three arguments.
#[test]
fn test_range_with_step_has_three_arguments() {
    use decypher::ast::expr::Expression;

    let query = parse("RETURN range(0, 10, 2) AS r;").unwrap();
    let QueryBody::SingleQuery(sq) = &query.statements[0] else {
        panic!("expected SingleQuery");
    };
    let decypher::ast::query::SingleQueryKind::SinglePart(spq) = &sq.kind else {
        panic!("expected SinglePart query");
    };
    let SinglePartBody::Return(ret) = &spq.body else {
        panic!("expected Return body");
    };
    match &ret.items[0].expression {
        Expression::FunctionCall(fi) => {
            check!(fi.arguments.len() == 3);
        }
        other => panic!("expected a FunctionCall, got {other:?}"),
    }
}

/// `range` remains usable as an ordinary variable name (it is only a contextual
/// keyword). Adding it to the expression function-call arm must not regress
/// this.
///
/// Unit: `parse()`
/// Precondition: `WITH 1 AS range RETURN range;`.
/// Expectation: parser returns `Ok`.
#[test]
fn test_range_is_still_a_valid_variable_name() {
    let result = parse("WITH 1 AS range RETURN range;");
    check!(result.is_ok(), "{:?}", result.err());
}

// ============================================================
// Compound-expression truncation regressions
//
// The CST for a compound expression like `1 + 1` stores the operator node
// (`ADD_SUB_EXPR`) as a *sibling* of its LHS, not a wrapper around it (the
// LHS is recovered via `prev_sibling()`). Several typed-CST accessors that
// pick "an" Expression out of a run of such siblings previously grabbed the
// *first* castable node (the LHS atom) instead of the *last* (the fully
// composed expression), truncating every compound expression down to its
// leading atom wherever they were used: list-literal elements, map-literal
// values, the UNWIND source expression, and (via an unrelated but
// same-shaped bug) FunctionInvocation arguments dropping a bare-variable
// first argument entirely.
// ============================================================

fn first_projection_expr(query: &decypher::ast::Query) -> &decypher::ast::expr::Expression {
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

/// `RETURN [1 + 1]` must parse as a *single* list element that is the
/// fully composed `1 + 1` binary expression — not two elements (`1` and
/// `1 + 1`) as a spurious duplicate of the leading atom.
///
/// Unit: `parse()` / AST `Literal::List`
/// Precondition: `RETURN [1 + 1];`.
/// Expectation: `elements.len() == 1` and that element is `BinaryOp { Add }`.
#[test]
fn test_list_literal_element_is_full_binary_expr() {
    use decypher::ast::expr::{BinaryOperator, Expression, Literal};

    let query = parse("RETURN [1 + 1];").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::List(list)) => {
            check!(list.elements.len() == 1);
            match &list.elements[0] {
                Expression::BinaryOp { op, .. } => {
                    check!(*op == BinaryOperator::Add);
                }
                other => panic!("expected BinaryOp element, got {other:?}"),
            }
        }
        other => panic!("expected a List literal, got {other:?}"),
    }
}

/// `RETURN [a.list[1]]` must parse as a single element that is the full
/// `a.list[1]` index expression (list = `a.list` PropertyLookup, not just
/// `a`).
///
/// Unit: `parse()` / AST `Literal::List`
/// Precondition: `RETURN [a.list[1]];`.
/// Expectation: `elements.len() == 1`; the element is `ListIndex` whose
/// `list` operand is a `PropertyLookup`.
#[test]
fn test_list_literal_element_nested_index() {
    use decypher::ast::expr::{Expression, Literal};

    let query = parse("RETURN [a.list[1]];").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::List(list)) => {
            check!(list.elements.len() == 1);
            match &list.elements[0] {
                Expression::ListIndex { list, .. } => match list.as_ref() {
                    Expression::PropertyLookup { .. } => {}
                    other => panic!("expected PropertyLookup base, got {other:?}"),
                },
                other => panic!("expected ListIndex element, got {other:?}"),
            }
        }
        other => panic!("expected a List literal, got {other:?}"),
    }
}

/// `RETURN {a: 1 + 2}` must parse with the entry value being the full
/// `1 + 2` binary expression, not truncated to `1`.
///
/// Unit: `parse()` / AST `Literal::Map`
/// Precondition: `RETURN {a: 1 + 2};`.
/// Expectation: one entry whose value is `BinaryOp { Add }`.
#[test]
fn test_map_literal_value_is_full_binary_expr() {
    use decypher::ast::expr::{BinaryOperator, Expression, Literal};

    let query = parse("RETURN {a: 1 + 2};").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::Map(map)) => {
            check!(map.entries.len() == 1);
            match &map.entries[0].1 {
                Expression::BinaryOp { op, .. } => {
                    check!(*op == BinaryOperator::Add);
                }
                other => panic!("expected BinaryOp value, got {other:?}"),
            }
        }
        other => panic!("expected a Map literal, got {other:?}"),
    }
}

/// `RETURN {k: n.prop}` must parse with the entry value being the full
/// `n.prop` property lookup, not truncated to the bare variable `n`.
///
/// Unit: `parse()` / AST `Literal::Map`
/// Precondition: `RETURN {k: n.prop};`.
/// Expectation: one entry whose value is `PropertyLookup`.
#[test]
fn test_map_literal_value_property_lookup() {
    use decypher::ast::expr::{Expression, Literal};

    let query = parse("RETURN {k: n.prop};").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::Map(map)) => {
            check!(map.entries.len() == 1);
            match &map.entries[0].1 {
                Expression::PropertyLookup { .. } => {}
                other => panic!("expected PropertyLookup value, got {other:?}"),
            }
        }
        other => panic!("expected a Map literal, got {other:?}"),
    }
}

/// `UNWIND n.list AS x` must bind the *source* expression to the full
/// `n.list` property lookup and the *bound variable* to `x` — not conflate
/// both to the leading atom `n` of the source expression.
///
/// Unit: `parse()` / AST `ReadingClause::Unwind`
/// Precondition: `UNWIND n.list AS x RETURN x;`.
/// Expectation: `expression` is `PropertyLookup`; `variable.name.name == "x"`.
#[test]
fn test_unwind_expression_and_variable_are_not_conflated() {
    use decypher::ast::expr::Expression;
    use decypher::ast::query::ReadingClause;

    let query = parse("UNWIND n.list AS x RETURN x;").unwrap();
    let QueryBody::SingleQuery(sq) = &query.statements[0] else {
        panic!("expected SingleQuery");
    };
    let decypher::ast::query::SingleQueryKind::SinglePart(spq) = &sq.kind else {
        panic!("expected SinglePart query");
    };
    check!(spq.reading_clauses.len() == 1);
    match &spq.reading_clauses[0] {
        ReadingClause::Unwind(unwind) => {
            match &unwind.expression {
                Expression::PropertyLookup { .. } => {}
                other => panic!("expected PropertyLookup source, got {other:?}"),
            }
            check!(unwind.variable.name.name == "x");
        }
        other => panic!("expected Unwind reading clause, got {other:?}"),
    }
}

/// `coalesce(x, 1)` must keep the bare-variable first argument `x` — it must
/// not be silently dropped because it happens to be a `VARIABLE` CST node
/// (which older code mistook for a leftover callee-name fragment).
///
/// Unit: `parse()` / AST `Expression::FunctionCall`
/// Precondition: `RETURN coalesce(x, 1);`.
/// Expectation: `arguments.len() == 2`; the first argument is `Variable("x")`.
#[test]
fn test_function_call_keeps_bare_variable_argument() {
    use decypher::ast::expr::Expression;

    let query = parse("RETURN coalesce(x, 1);").unwrap();
    match first_projection_expr(&query) {
        Expression::FunctionCall(fi) => {
            check!(fi.arguments.len() == 2);
            match &fi.arguments[0] {
                Expression::Variable(v) => {
                    check!(v.name.name == "x");
                }
                other => panic!("expected Variable argument, got {other:?}"),
            }
        }
        other => panic!("expected a FunctionCall, got {other:?}"),
    }
}

// ============================================================
// List-comprehension / quantifier shape regressions
//
// `[x IN list [WHERE pred] [| map]]` parses to a `FILTER_EXPRESSION` (holding
// `ID_IN_COLL` = variable + collection, and an optional `WHERE_CLAUSE`)
// nested inside `LIST_COMPREHENSION`, with an optional trailing `| map`
// expression as its sibling. Two bugs conspired to make this unusable:
//
// 1. A parser bug closed the `FILTER_EXPRESSION` node one token too early —
//    right after the collection expression — so `WHERE_CLAUSE` ended up as a
//    *sibling* of `FILTER_EXPRESSION` (under `LIST_COMPREHENSION`) instead of
//    nested inside it, silently detaching the predicate from every accessor
//    that looked for it in the (correct, intended) nested position.
// 2. The typed-AST `ListComprehension` had no `collection` field at all (the
//    accessor computed it and then discarded it), and its `body()`/map
//    accessor mistook the (Expression-castable) `FILTER_EXPRESSION` node
//    itself for the map expression whenever no `| map` was present.
//
// `all/any/none/single(x IN list WHERE pred)` parse as a plain
// `FUNCTION_INVOCATION` — decypher has no dedicated quantifier grammar node.
// But the binder, `IN`, collection, `WHERE`, and predicate are all still
// present as flat children/tokens of that node (unlike an ordinary call's
// comma-separated arguments, `parse_filter_like_expr` bumps bare `KW_IN` /
// `KW_WHERE` tokens directly instead of wrapping them), so `arguments()` can
// — and now does — segment on those boundary tokens too, recovering the
// binder, collection, and predicate as three separate positional arguments
// instead of collapsing them into one mangled trailing expression.
// ============================================================

/// `[x IN [1,2,3] WHERE x > 1]` (WHERE, no map) must expose the full
/// collection and the WHERE predicate, and must not fabricate a map.
///
/// Unit: `parse()` / AST `Expression::ListComprehension`
/// Precondition: `RETURN [x IN [1,2,3] WHERE x > 1];`.
/// Expectation: `collection` is a 3-element list, `filter` is `Some(x > 1)`,
/// `map` is `None`.
#[test]
fn test_list_comprehension_where_no_map() {
    use decypher::ast::expr::{Expression, Literal};

    let query = parse("RETURN [x IN [1,2,3] WHERE x > 1];").unwrap();
    match first_projection_expr(&query) {
        Expression::ListComprehension(lc) => {
            check!(lc.variable.name.name == "x");
            match lc.collection.as_ref() {
                Expression::Literal(Literal::List(list)) => {
                    check!(list.elements.len() == 3);
                }
                other => panic!("expected List collection, got {other:?}"),
            }
            match lc.filter.as_deref() {
                Some(Expression::Comparison { .. }) => {}
                other => panic!("expected Some(Comparison) filter, got {other:?}"),
            }
            check!(lc.map.is_none());
        }
        other => panic!("expected a ListComprehension, got {other:?}"),
    }
}

/// `[x IN [1,2,3] WHERE x > 1 | x*2]` (WHERE and map) must expose all three
/// of collection, filter, and map simultaneously.
///
/// Unit: `parse()` / AST `Expression::ListComprehension`
/// Precondition: `RETURN [x IN [1,2,3] WHERE x > 1 | x*2];`.
/// Expectation: `collection` is a 3-element list, `filter` is
/// `Some(Comparison)`, `map` is `Some(BinaryOp)`.
#[test]
fn test_list_comprehension_where_and_map() {
    use decypher::ast::expr::{Expression, Literal};

    let query = parse("RETURN [x IN [1,2,3] WHERE x > 1 | x*2];").unwrap();
    match first_projection_expr(&query) {
        Expression::ListComprehension(lc) => {
            match lc.collection.as_ref() {
                Expression::Literal(Literal::List(list)) => {
                    check!(list.elements.len() == 3);
                }
                other => panic!("expected List collection, got {other:?}"),
            }
            match lc.filter.as_deref() {
                Some(Expression::Comparison { .. }) => {}
                other => panic!("expected Some(Comparison) filter, got {other:?}"),
            }
            match &lc.map {
                Some(Expression::BinaryOp { .. }) => {}
                other => panic!("expected Some(BinaryOp) map, got {other:?}"),
            }
        }
        other => panic!("expected a ListComprehension, got {other:?}"),
    }
}

/// `[x IN [1,2,3] | x*2]` (map, no WHERE) must expose the collection and map,
/// with `filter` correctly `None` (not a mangled quantifier-shaped node).
///
/// Unit: `parse()` / AST `Expression::ListComprehension`
/// Precondition: `RETURN [x IN [1,2,3] | x*2];`.
/// Expectation: `collection` is a 3-element list, `filter` is `None`, `map`
/// is `Some(BinaryOp)`.
#[test]
fn test_list_comprehension_map_no_where() {
    use decypher::ast::expr::{Expression, Literal};

    let query = parse("RETURN [x IN [1,2,3] | x*2];").unwrap();
    match first_projection_expr(&query) {
        Expression::ListComprehension(lc) => {
            match lc.collection.as_ref() {
                Expression::Literal(Literal::List(list)) => {
                    check!(list.elements.len() == 3);
                }
                other => panic!("expected List collection, got {other:?}"),
            }
            check!(lc.filter.is_none());
            match &lc.map {
                Some(Expression::BinaryOp { .. }) => {}
                other => panic!("expected Some(BinaryOp) map, got {other:?}"),
            }
        }
        other => panic!("expected a ListComprehension, got {other:?}"),
    }
}

/// `all(x IN [1,2,3] WHERE x > 1)` must recover the binder, collection, and
/// predicate as three separate positional arguments, instead of collapsing
/// them into a single mangled trailing expression.
///
/// Unit: `parse()` / AST `Expression::FunctionCall`
/// Precondition: `RETURN all(x IN [1,2,3] WHERE x > 1);`.
/// Expectation: `arguments.len() == 3`: `Variable("x")`, a 3-element list,
/// then a `Comparison`.
#[test]
fn test_all_quantifier_recovers_binder_collection_predicate() {
    use decypher::ast::expr::{Expression, Literal};

    let query = parse("RETURN all(x IN [1,2,3] WHERE x > 1);").unwrap();
    match first_projection_expr(&query) {
        Expression::FunctionCall(fi) => {
            check!(fi.name.len() == 1);
            check!(fi.name[0].name == "all");
            check!(fi.arguments.len() == 3);
            match &fi.arguments[0] {
                Expression::Variable(v) => {
                    check!(v.name.name == "x");
                }
                other => panic!("expected Variable binder, got {other:?}"),
            }
            match &fi.arguments[1] {
                Expression::Literal(Literal::List(list)) => {
                    check!(list.elements.len() == 3);
                }
                other => panic!("expected List collection, got {other:?}"),
            }
            match &fi.arguments[2] {
                Expression::Comparison { .. } => {}
                other => panic!("expected Comparison predicate, got {other:?}"),
            }
        }
        other => panic!("expected a FunctionCall, got {other:?}"),
    }
}

/// `any(x IN [1,2,3] WHERE x > 1)` — same shape as `all`, different keyword.
///
/// Unit: `parse()` / AST `Expression::FunctionCall`
/// Precondition: `RETURN any(x IN [1,2,3] WHERE x > 1);`.
/// Expectation: `arguments.len() == 3`, same binder/collection/predicate shape.
#[test]
fn test_any_quantifier_recovers_binder_collection_predicate() {
    use decypher::ast::expr::{Expression, Literal};

    let query = parse("RETURN any(x IN [1,2,3] WHERE x > 1);").unwrap();
    match first_projection_expr(&query) {
        Expression::FunctionCall(fi) => {
            check!(fi.name[0].name == "any");
            check!(fi.arguments.len() == 3);
            match &fi.arguments[0] {
                Expression::Variable(v) => {
                    check!(v.name.name == "x");
                }
                other => panic!("expected Variable binder, got {other:?}"),
            }
            match &fi.arguments[1] {
                Expression::Literal(Literal::List(list)) => {
                    check!(list.elements.len() == 3);
                }
                other => panic!("expected List collection, got {other:?}"),
            }
            match &fi.arguments[2] {
                Expression::Comparison { .. } => {}
                other => panic!("expected Comparison predicate, got {other:?}"),
            }
        }
        other => panic!("expected a FunctionCall, got {other:?}"),
    }
}

/// An ordinary function call unrelated to the quantifier shape must be
/// unaffected by the new KW_IN/KW_WHERE/PIPE segment-boundary logic: a bare
/// `x IN list` boolean-membership argument composes into a single
/// `LIST_OP_EXPR`-backed expression (its `KW_IN` token is nested inside that
/// node, not a direct child of `FUNCTION_INVOCATION`), so it must still come
/// through as exactly one argument.
///
/// Unit: `parse()` / AST `Expression::FunctionCall`
/// Precondition: `RETURN coalesce(x IN list, 1);`.
/// Expectation: `arguments.len() == 2` (the `IN` expression, then `1`) — not
/// 3.
#[test]
fn test_function_call_in_expression_argument_not_split() {
    use decypher::ast::expr::Expression;

    let query = parse("RETURN coalesce(x IN list, 1);").unwrap();
    match first_projection_expr(&query) {
        Expression::FunctionCall(fi) => {
            check!(fi.arguments.len() == 2);
        }
        other => panic!("expected a FunctionCall, got {other:?}"),
    }
}

/// A bare-identifier collection (`x IN coll`, as opposed to a list literal or
/// property lookup) must still parse. `IdInColl::collection()` used to
/// exclude *every* `VARIABLE`-kind child to skip the binder, which also threw
/// away a same-kind bare-identifier collection, leaving no Expression-castable
/// child at all — surfaced as an "missing collection in list comp" internal
/// parse error.
///
/// Unit: `parse()` / AST `Expression::ListComprehension`
/// Precondition: `RETURN [x IN coll | x*2];`.
/// Expectation: `parse()` succeeds and `collection` is `Variable("coll")`.
#[test]
fn test_list_comprehension_bare_identifier_collection() {
    use decypher::ast::expr::Expression;

    let query = parse("RETURN [x IN coll | x*2];").unwrap();
    match first_projection_expr(&query) {
        Expression::ListComprehension(lc) => match lc.collection.as_ref() {
            Expression::Variable(v) => {
                check!(v.name.name == "coll");
            }
            other => panic!("expected Variable collection, got {other:?}"),
        },
        other => panic!("expected a ListComprehension, got {other:?}"),
    }
}

// ============================================================
// Pattern predicates & bare label-checks in expression position
//
// A bare relationship/node pattern used directly as a boolean expression
// (`WHERE (n)-[:REL]->()`) and a bare label-check (`WHERE x:Label`) are
// both valid openCypher expressions. decypher's `(` primary-expression
// disambiguation (`looks_like_relationships_pattern`, used to decide
// between a `RelationshipsPattern` atom and a `ParenthesizedExpr`) had an
// off-by-one bug: it assumed the lexer clone still had the `(` ahead of
// it, but by the time `parse_atom` dispatches on `SyntaxKind::L_PAREN`, the
// parser's internal lexer cursor has already advanced past that token. The
// resulting misaligned scan both missed genuine patterns (`(n)-[:REL]->()`
// parsed as a truncated grouped expression, silently dropping the WHERE
// filter) and misfired on ordinary grouped arithmetic followed by a binary
// `-` (`(-3) - 2`, `(4 ^ 3) - 1`), mistaking the trailing subtraction for
// the start of a relationship chain.
// ============================================================

fn match_where_clause(query: &decypher::ast::Query) -> &decypher::ast::expr::Expression {
    let QueryBody::SingleQuery(sq) = &query.statements[0] else {
        panic!("expected SingleQuery");
    };
    let decypher::ast::query::SingleQueryKind::SinglePart(spq) = &sq.kind else {
        panic!("expected SinglePart query");
    };
    let decypher::ast::query::ReadingClause::Match(m) = &spq.reading_clauses[0] else {
        panic!("expected Match clause");
    };
    m.where_clause
        .as_ref()
        .unwrap_or_else(|| panic!("expected a WHERE clause"))
}

/// `WHERE (n)-[:REL]->()` — a bare relationship pattern in expression
/// position — must parse as an `Expression::Pattern(RelationshipsPattern)`
/// whose start node is `n` and which has one relationship chain, not be
/// silently dropped (`where_clause: None`) or misparsed as arithmetic.
///
/// Unit: `parse()` / AST `Expression::Pattern`
/// Precondition: `MATCH (n) WHERE (n)-[:REL]->() RETURN n;`.
/// Expectation: `where_clause` is `Pattern` with `start.variable == "n"` and
///   `chains.len() == 1`.
#[test]
fn test_pattern_predicate_in_where_clause() {
    use decypher::ast::expr::Expression;

    let query = parse("MATCH (n) WHERE (n)-[:REL]->() RETURN n;").unwrap();
    match match_where_clause(&query) {
        Expression::Pattern(rp) => {
            check!(rp.start.variable.as_ref().unwrap().name.name == "n");
            check!(rp.chains.len() == 1);
        }
        other => panic!("expected Expression::Pattern, got {other:?}"),
    }
}

/// `WHERE NOT (a)-->(b)` — a pattern predicate under a unary `NOT` — must
/// keep the pattern as the `NOT`'s operand rather than misfiring the
/// `(`-vs-grouped-expression heuristic (which previously produced garbage
/// arithmetic from the `-->` tokens).
///
/// Unit: `parse()` / AST `Expression::UnaryOp` / `Expression::Pattern`
/// Precondition: `MATCH (a) WHERE NOT (a)-->(b) RETURN a;`.
/// Expectation: `where_clause` is `UnaryOp { op: Not, operand: Pattern(..) }`.
#[test]
fn test_not_pattern_predicate() {
    use decypher::ast::expr::{Expression, UnaryOperator};

    let query = parse("MATCH (a) WHERE NOT (a)-->(b) RETURN a;").unwrap();
    match match_where_clause(&query) {
        Expression::UnaryOp { op, operand, .. } => {
            check!(*op == UnaryOperator::Not);
            match operand.as_ref() {
                Expression::Pattern(rp) => {
                    check!(rp.chains.len() == 1);
                }
                other => panic!("expected Pattern operand, got {other:?}"),
            }
        }
        other => panic!("expected Expression::UnaryOp, got {other:?}"),
    }
}

/// A pattern predicate followed by a trailing boolean operator —
/// `(n)-[:R]->() AND n.x > 1` — must compose the pattern as the `AND`'s
/// left-hand side rather than being truncated at the pattern's closing
/// `)`. This is the same root cause as the plain pattern-predicate case:
/// once `(n)-[:R]->()` parses as a single primary expression (an atom),
/// the surrounding Pratt loop naturally continues into `AND n.x > 1` like
/// any other atom.
///
/// Unit: `parse()` / AST `Expression::BinaryOp`
/// Precondition: `MATCH (n) WHERE (n)-[:R]->() AND n.x > 1 RETURN n;`.
/// Expectation: `where_clause` is `BinaryOp { op: And, lhs: Pattern(..), .. }`.
#[test]
fn test_pattern_predicate_with_trailing_and() {
    use decypher::ast::expr::{BinaryOperator, Expression};

    let query = parse("MATCH (n) WHERE (n)-[:R]->() AND n.x > 1 RETURN n;").unwrap();
    match match_where_clause(&query) {
        Expression::BinaryOp { op, lhs, .. } => {
            check!(*op == BinaryOperator::And);
            match lhs.as_ref() {
                Expression::Pattern(_) => {}
                other => panic!("expected Pattern lhs, got {other:?}"),
            }
        }
        other => panic!("expected Expression::BinaryOp, got {other:?}"),
    }
}

/// `WHERE x:Label` — a bare node-variable label test used as a boolean
/// expression (not inside a MATCH pattern) — must parse as
/// `Expression::NodeLabels`.
///
/// Unit: `parse()` / AST `Expression::NodeLabels`
/// Precondition: `MATCH (n) WHERE x:Label RETURN n;`.
/// Expectation: `where_clause` is `NodeLabels { base: Variable("x"), .. }`.
#[test]
fn test_bare_label_check_expression() {
    use decypher::ast::expr::Expression;

    let query = parse("MATCH (n) WHERE x:Label RETURN n;").unwrap();
    match match_where_clause(&query) {
        Expression::NodeLabels { base, labels, .. } => {
            match base.as_ref() {
                Expression::Variable(v) => {
                    check!(v.name.name == "x");
                }
                other => panic!("expected Variable base, got {other:?}"),
            }
            check!(labels.len() == 1);
        }
        other => panic!("expected Expression::NodeLabels, got {other:?}"),
    }
}

/// A bare label-check whose label name is a non-reserved (contextual)
/// keyword — e.g. `TYPE` — must still parse. `is_label_check_follow` used
/// to gate the postfix `:Label` production on a fixed set of token kinds
/// that excluded contextual keywords, even though `parse_label_atom`
/// itself already accepted them (matching NodePattern label position).
///
/// Unit: `parse()` / AST `Expression::NodeLabels`
/// Precondition: `MATCH (m) RETURN m:TYPE;`.
/// Expectation: `parse()` succeeds; the projection expression is
///   `NodeLabels` with label name `"TYPE"`.
#[test]
fn test_label_check_with_contextual_keyword() {
    use decypher::ast::expr::Expression;
    use decypher::ast::pattern::LabelExpression;

    let query = parse("MATCH (m) RETURN m:TYPE;").unwrap();
    match first_projection_expr(&query) {
        Expression::NodeLabels { labels, .. } => {
            check!(labels.len() == 1);
            match &labels[0] {
                LabelExpression::Static(name) => {
                    check!(name.name == "TYPE");
                }
                other => panic!("expected Static label, got {other:?}"),
            }
        }
        other => panic!("expected Expression::NodeLabels, got {other:?}"),
    }
}

/// `RETURN (1 + 2)` must still parse as a grouped arithmetic expression —
/// the pattern-predicate `(`-disambiguation must not misfire on an ordinary
/// parenthesized expression that contains no relationship chain at all.
///
/// Unit: `parse()` / AST `Expression::Parenthesized`
/// Precondition: `RETURN (1 + 2);`.
/// Expectation: `Parenthesized(BinaryOp { op: Add, .. })`.
#[test]
fn test_grouped_arithmetic_expr_unaffected() {
    use decypher::ast::expr::{BinaryOperator, Expression};

    let query = parse("RETURN (1 + 2);").unwrap();
    match first_projection_expr(&query) {
        Expression::Parenthesized(inner) => match inner.as_ref() {
            Expression::BinaryOp { op, .. } => {
                check!(*op == BinaryOperator::Add);
            }
            other => panic!("expected BinaryOp inside parens, got {other:?}"),
        },
        other => panic!("expected Expression::Parenthesized, got {other:?}"),
    }
}

/// `RETURN (-3) - 2` must parse as subtraction of `2` from the grouped
/// `-3`, not be misdetected as a pattern (the old heuristic saw the `)`
/// immediately followed by `-` and treated it as the start of a
/// relationship chain, since it never checked what came *after* that `-`).
///
/// Unit: `parse()` / AST `Expression::BinaryOp`
/// Precondition: `RETURN (-3) - 2;`.
/// Expectation: `BinaryOp { op: Subtract, lhs: Parenthesized(..), .. }`.
#[test]
fn test_parenthesized_unary_minus_then_subtraction_unaffected() {
    use decypher::ast::expr::{BinaryOperator, Expression};

    let query = parse("RETURN (-3) - 2;").unwrap();
    match first_projection_expr(&query) {
        Expression::BinaryOp { op, lhs, .. } => {
            check!(*op == BinaryOperator::Subtract);
            match lhs.as_ref() {
                Expression::Parenthesized(_) => {}
                other => panic!("expected Parenthesized lhs, got {other:?}"),
            }
        }
        other => panic!("expected Expression::BinaryOp, got {other:?}"),
    }
}

/// A normal `MATCH` pattern with a relationship chain — as opposed to a
/// bare pattern used as an expression — must still parse as a genuine
/// `PatternElement::Path` with one chain. `MATCH`-clause pattern parsing
/// never goes through the `(`-vs-grouped-expression primary-expression
/// disambiguation at all, so this is unaffected by construction, but is
/// worth pinning down explicitly.
///
/// Unit: `parse()` / AST `PatternElement::Path`
/// Precondition: `MATCH (n)-[:R]->(m) RETURN n;`.
/// Expectation: one pattern part whose `Path` has `chains.len() == 1`.
#[test]
fn test_normal_match_pattern_with_chain_unaffected() {
    let query = parse("MATCH (n)-[:R]->(m) RETURN n;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            decypher::ast::query::SingleQueryKind::SinglePart(spq) => {
                match &spq.reading_clauses[0] {
                    decypher::ast::query::ReadingClause::Match(m) => {
                        check!(m.pattern.parts.len() == 1);
                        match &m.pattern.parts[0].anonymous.element {
                            decypher::ast::pattern::PatternElement::Path { start, chains } => {
                                check!(start.variable.as_ref().unwrap().name.name == "n");
                                check!(chains.len() == 1);
                            }
                            _ => panic!("expected Path pattern element"),
                        }
                    }
                    _ => panic!("expected Match clause"),
                }
            }
            _ => panic!("expected SinglePart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

// ── Multi-part query ending in an updating clause, no trailing RETURN ──
//
// openCypher allows a multi-part (`WITH`-joined) query to end in a run of
// updating clauses with no trailing `RETURN`/`FINISH` — the same shape
// `SinglePartBody::Updating { return_clause: None, .. }` already represents
// for single-part writes. Previously the multi-part AST builder required a
// `RETURN`/`FINISH`/final-`WITH` to set `final_part` and raised
// `Internal("multi-part query missing final part")` otherwise.

/// `MATCH (a) WITH a CREATE (a)-[:R]->()` — a multi-part query whose final
/// part is a bare `CREATE` with no `RETURN` — must parse, with the `CREATE`
/// exposed on `final_part.body` as `SinglePartBody::Updating` and
/// `return_clause == None`.
///
/// Unit: `parse()` / AST `MultiPartQuery::final_part`
/// Precondition: `MATCH (a) WITH a CREATE (a)-[:R]->();`.
/// Expectation: one intermediate part (the `MATCH … WITH a`), and
/// `final_part.body` is `Updating { updating: [Create(_)], return_clause: None }`.
#[test]
fn test_multipart_final_updating_no_return() {
    use decypher::ast::query::{SingleQueryKind, UpdatingClause};

    let query = parse("MATCH (a) WITH a CREATE (a)-[:R]->();").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            SingleQueryKind::MultiPart(mpq) => {
                check!(mpq.parts.len() == 1);
                match &mpq.final_part.body {
                    SinglePartBody::Updating {
                        updating,
                        return_clause,
                    } => {
                        check!(updating.len() == 1);
                        check!(matches!(updating[0], UpdatingClause::Create(_)));
                        check!(return_clause.is_none());
                    }
                    other => panic!("expected Updating final-part body, got {other:?}"),
                }
            }
            _ => panic!("expected MultiPart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// `UNWIND [1,2] AS x CREATE (:N {v:x})` has no `WITH`, so it is a
/// single-part query — the updating-with-no-RETURN shape it needs
/// (`SinglePartBody::Updating { return_clause: None, .. }`) already existed
/// before this fix; pinned here as a sibling regression case to the
/// multi-part fix above.
///
/// Unit: `parse()` / AST `SinglePartBody`
/// Precondition: `UNWIND [1,2] AS x CREATE (:N {v:x});`.
/// Expectation: `SinglePart` query, `Updating` body with one `Create` clause
/// and no `RETURN`.
#[test]
fn test_unwind_create_no_with_has_updating_body() {
    use decypher::ast::query::{SingleQueryKind, UpdatingClause};

    let query = parse("UNWIND [1,2] AS x CREATE (:N {v:x});").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            SingleQueryKind::SinglePart(spq) => {
                check!(spq.reading_clauses.len() == 1);
                match &spq.body {
                    SinglePartBody::Updating {
                        updating,
                        return_clause,
                    } => {
                        check!(updating.len() == 1);
                        check!(matches!(updating[0], UpdatingClause::Create(_)));
                        check!(return_clause.is_none());
                    }
                    other => panic!("expected Updating body, got {other:?}"),
                }
            }
            _ => panic!("expected SinglePart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// `MATCH (n) WITH n DELETE n` — a multi-part query ending in `DELETE`
/// with no `RETURN` — must parse the same way as the `CREATE` case above.
///
/// Unit: `parse()` / AST `MultiPartQuery::final_part`
/// Precondition: `MATCH (n) WITH n DELETE n;`.
/// Expectation: `final_part.body` is `Updating { updating: [Delete(_)],
/// return_clause: None }`.
#[test]
fn test_multipart_match_with_delete_no_return() {
    use decypher::ast::query::{SingleQueryKind, UpdatingClause};

    let query = parse("MATCH (n) WITH n DELETE n;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            SingleQueryKind::MultiPart(mpq) => match &mpq.final_part.body {
                SinglePartBody::Updating {
                    updating,
                    return_clause,
                } => {
                    check!(updating.len() == 1);
                    check!(matches!(updating[0], UpdatingClause::Delete(_)));
                    check!(return_clause.is_none());
                }
                other => panic!("expected Updating final-part body, got {other:?}"),
            },
            _ => panic!("expected MultiPart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// Unaffected: `MATCH (a) WITH a RETURN a` — a multi-part query whose final
/// part still ends in `RETURN` — must still parse the same as before, with
/// `final_part.body` as `SinglePartBody::Return`.
///
/// Unit: `parse()` / AST `MultiPartQuery::final_part`
/// Precondition: `MATCH (a) WITH a RETURN a;`.
/// Expectation: `final_part.body` is `SinglePartBody::Return(_)`.
#[test]
fn test_multipart_ending_in_return_unaffected() {
    use decypher::ast::query::SingleQueryKind;

    let query = parse("MATCH (a) WITH a RETURN a;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            SingleQueryKind::MultiPart(mpq) => {
                check!(matches!(mpq.final_part.body, SinglePartBody::Return(_)));
            }
            _ => panic!("expected MultiPart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// Unaffected: single-part `CREATE (n) RETURN n` — updating clause followed
/// by `RETURN` in the same (non-`WITH`-joined) part — must still parse.
///
/// Unit: `parse()` / AST `SinglePartBody`
/// Precondition: `CREATE (n) RETURN n;`.
/// Expectation: `SinglePart` query, `Updating` body with one `Create` and
/// `return_clause.is_some()`.
#[test]
fn test_single_part_create_return_unaffected() {
    use decypher::ast::query::SingleQueryKind;

    let query = parse("CREATE (n) RETURN n;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            SingleQueryKind::SinglePart(spq) => match &spq.body {
                SinglePartBody::Updating {
                    updating,
                    return_clause,
                } => {
                    check!(updating.len() == 1);
                    check!(return_clause.is_some());
                }
                other => panic!("expected Updating body, got {other:?}"),
            },
            _ => panic!("expected SinglePart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

// ── SET on a parenthesized-expression target ────────────────────────────
//
// openCypher's `PropertyExpression` grammar is `Atom (PropertyLookup)+`,
// and `Atom` includes a parenthesized group — so `(n).prop` is a valid
// property-mutation target, same as the bare `n.prop`. Previously
// `parse_set_item` only accepted a bare variable as the SET target.

/// `SET (n).prop = 1` — a parenthesized node expression as the SET
/// target — must parse as `SetItem::Property` whose `property` is a
/// `PropertyLookup` with a `Parenthesized(Variable(n))` base.
///
/// Unit: `parse()` / AST `SetItem`
/// Precondition: `MATCH (n) SET (n).prop = 1;`.
/// Expectation: one `SetItem::Property` whose `property` is
/// `Expression::PropertyLookup { base: Expression::Parenthesized(box
/// Expression::Variable(n)), property: "prop" }`.
#[test]
fn test_set_parenthesized_target() {
    use decypher::ast::clause::SetItem;
    use decypher::ast::expr::Expression;
    use decypher::ast::query::{ReadingClause, SingleQueryKind, UpdatingClause};

    let query = parse("MATCH (n) SET (n).prop = 1;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            SingleQueryKind::SinglePart(spq) => {
                check!(matches!(spq.reading_clauses[0], ReadingClause::Match(_)));
                match &spq.body {
                    SinglePartBody::Updating { updating, .. } => {
                        check!(updating.len() == 1);
                        match &updating[0] {
                            UpdatingClause::Set(set) => {
                                check!(set.items.len() == 1);
                                match &set.items[0] {
                                    SetItem::Property { property, .. } => match property {
                                        Expression::PropertyLookup { base, property, .. } => {
                                            check!(property.name.name == "prop");
                                            check!(matches!(
                                                base.as_ref(),
                                                Expression::Parenthesized(inner)
                                                    if matches!(inner.as_ref(), Expression::Variable(v) if v.name.name == "n")
                                            ));
                                        }
                                        other => panic!("expected PropertyLookup, got {other:?}"),
                                    },
                                    other => panic!("expected SetItem::Property, got {other:?}"),
                                }
                            }
                            other => panic!("expected UpdatingClause::Set, got {other:?}"),
                        }
                    }
                    other => panic!("expected Updating body, got {other:?}"),
                }
            }
            _ => panic!("expected SinglePart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// Unaffected: `SET n.prop = 1` (bare variable target, no parens) — must
/// still parse as `SetItem::Property` with a bare `Variable` base (not
/// wrapped in `Parenthesized`).
///
/// Unit: `parse()` / AST `SetItem`
/// Precondition: `MATCH (n) SET n.prop = 1;`.
/// Expectation: `SetItem::Property` whose `property` base is
/// `Expression::Variable(n)` directly.
#[test]
fn test_set_bare_variable_target_unaffected() {
    use decypher::ast::clause::SetItem;
    use decypher::ast::expr::Expression;
    use decypher::ast::query::{SingleQueryKind, UpdatingClause};

    let query = parse("MATCH (n) SET n.prop = 1;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            SingleQueryKind::SinglePart(spq) => match &spq.body {
                SinglePartBody::Updating { updating, .. } => match &updating[0] {
                    UpdatingClause::Set(set) => match &set.items[0] {
                        SetItem::Property { property, .. } => match property {
                            Expression::PropertyLookup { base, .. } => {
                                check!(
                                    matches!(base.as_ref(), Expression::Variable(v) if v.name.name == "n")
                                );
                            }
                            other => panic!("expected PropertyLookup, got {other:?}"),
                        },
                        other => panic!("expected SetItem::Property, got {other:?}"),
                    },
                    other => panic!("expected UpdatingClause::Set, got {other:?}"),
                },
                other => panic!("expected Updating body, got {other:?}"),
            },
            _ => panic!("expected SinglePart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}
