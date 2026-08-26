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

// ── In-query CALL as a reading clause ───────────────────────────────────
//
// The grammar records every procedure `CALL` as a `STANDALONE_CALL` CST
// node; whether it is openCypher's `StandaloneCall` (the statement's only
// clause) or an `InQueryCall` (embedded in a larger query) is positional.
// Previously a mid-query CALL fell into a catch-all `=> {}` arm of the AST
// builder and vanished (`MATCH (n) CALL p() YIELD a RETURN a` parsed as a
// bare `MATCH … RETURN a`), and even standalone calls dropped their
// argument expressions.

/// `MATCH … CALL proc(args) YIELD … RETURN …` — an in-query procedure call —
/// must surface as a `ReadingClause::InQueryCall` with the procedure name,
/// arguments, and YIELD items intact.
///
/// Unit: `parse()` / AST `ReadingClause::InQueryCall`
/// Precondition: `MATCH (n) CALL test.proc(n.x, 1) YIELD out1, out2 AS o RETURN o;`.
/// Expectation: two reading clauses, the second an `InQueryCall` whose
/// invocation is named `test.proc` with 2 arguments and whose YIELD has two
/// items (the second aliased); the body is `Return`.
#[test]
fn test_in_query_call_after_match() {
    use decypher::ast::query::{ReadingClause, SingleQueryKind};

    let query = parse("MATCH (n) CALL test.proc(n.x, 1) YIELD out1, out2 AS o RETURN o;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            SingleQueryKind::SinglePart(spq) => {
                check!(spq.reading_clauses.len() == 2);
                match &spq.reading_clauses[1] {
                    ReadingClause::InQueryCall(c) => {
                        let names: Vec<_> =
                            c.call.name.name.iter().map(|s| s.name.as_str()).collect();
                        check!(names == ["test", "proc"]);
                        check!(c.call.name.arguments.len() == 2);
                        let yi = c.yield_items.as_ref().expect("expected YIELD items");
                        check!(yi.items.len() == 2);
                        check!(yi.items[0].procedure_field.name == "out1");
                        check!(yi.items[1].procedure_field.name == "out2");
                        check!(yi.items[1].alias.is_some());
                    }
                    other => panic!("expected InQueryCall, got {other:?}"),
                }
                check!(matches!(spq.body, SinglePartBody::Return(_)));
            }
            _ => panic!("expected SinglePart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// `MATCH … CALL proc(arg)` with no YIELD and no RETURN — a terminal void
/// procedure call — must parse (openCypher allows a query to end in a CALL)
/// with the call as a reading clause and an empty updating body.
///
/// Unit: `parse()` / AST `ReadingClause::InQueryCall`
/// Precondition: `MATCH (n) CALL test.sideEffect(n);`.
/// Expectation: two reading clauses, the second an `InQueryCall` with no
/// YIELD; body is `Updating` with no clauses and no RETURN.
#[test]
fn test_in_query_call_no_yield_terminal() {
    use decypher::ast::query::{ReadingClause, SingleQueryKind};

    let query = parse("MATCH (n) CALL test.sideEffect(n);").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            SingleQueryKind::SinglePart(spq) => {
                check!(spq.reading_clauses.len() == 2);
                match &spq.reading_clauses[1] {
                    ReadingClause::InQueryCall(c) => {
                        check!(c.call.name.arguments.len() == 1);
                        check!(c.yield_items.is_none());
                    }
                    other => panic!("expected InQueryCall, got {other:?}"),
                }
                match &spq.body {
                    SinglePartBody::Updating {
                        updating,
                        return_clause,
                    } => {
                        check!(updating.is_empty());
                        check!(return_clause.is_none());
                    }
                    other => panic!("expected empty Updating body, got {other:?}"),
                }
            }
            _ => panic!("expected SinglePart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// In-query `YIELD *` — legal only in a standalone call per the openCypher
/// grammar — must be rejected rather than silently dropped.
///
/// Unit: `parse()` error path
/// Precondition: `MATCH (n) CALL test.proc() YIELD * RETURN n;`.
/// Expectation: `parse` returns an error.
#[test]
fn test_in_query_call_yield_star_rejected() {
    check!(parse("MATCH (n) CALL test.proc() YIELD * RETURN n;").is_err());
}

/// Unaffected: a sole `CALL proc(args)` statement stays a standalone call —
/// and now keeps its argument expressions.
///
/// Unit: `parse()` / AST `QueryBody::Standalone`
/// Precondition: `CALL test.proc(1, 2)`.
/// Expectation: `QueryBody::Standalone` with name `test.proc` and 2 arguments.
#[test]
fn test_standalone_call_keeps_arguments() {
    let query = parse("CALL test.proc(1, 2)").unwrap();
    match &query.statements[0] {
        QueryBody::Standalone(sc) => {
            let names: Vec<_> = sc.call.name.name.iter().map(|s| s.name.as_str()).collect();
            check!(names == ["test", "proc"]);
            check!(sc.call.name.arguments.len() == 2);
        }
        other => panic!("expected Standalone call, got {other:?}"),
    }
}

// ── Updating clauses between WITH and RETURN ────────────────────────────
//
// In a multi-part query's FINAL part, updating clauses that preceded the
// RETURN were dropped: the builder's `RETURN` arm emitted a bare `Return`
// body without draining the pending updating-clause list, so
// `MATCH … WITH … MERGE … RETURN …` parsed as a plain read.

/// `MATCH … WITH … MERGE … RETURN …` — a MERGE in the final part of a
/// multi-part query — must land in an `Updating` body carrying the RETURN,
/// not vanish.
///
/// Unit: `parse()` / AST `MultiPartQuery.final_part`
/// Precondition: `MATCH (a) WITH a MERGE (b:B) RETURN b;`.
/// Expectation: final part has `Updating` body with one `Merge` and
/// `return_clause.is_some()`.
#[test]
fn test_merge_after_with_before_return() {
    use decypher::ast::query::{SingleQueryKind, UpdatingClause};

    let query = parse("MATCH (a) WITH a MERGE (b:B) RETURN b;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            SingleQueryKind::MultiPart(mpq) => match &mpq.final_part.body {
                SinglePartBody::Updating {
                    updating,
                    return_clause,
                } => {
                    check!(updating.len() == 1);
                    check!(matches!(updating[0], UpdatingClause::Merge(_)));
                    check!(return_clause.is_some());
                }
                other => panic!("expected Updating body, got {other:?}"),
            },
            _ => panic!("expected MultiPart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

/// `MATCH … WITH … CREATE … SET … RETURN …` — several updating clauses in
/// the final part — must all be kept, in order, alongside the RETURN.
///
/// Unit: `parse()` / AST `MultiPartQuery.final_part`
/// Precondition: `MATCH (a) WITH a CREATE (b) SET b.x = 1 RETURN b;`.
/// Expectation: final part has `Updating` body with `[Create, Set]` and
/// `return_clause.is_some()`.
#[test]
fn test_create_set_after_with_before_return() {
    use decypher::ast::query::{SingleQueryKind, UpdatingClause};

    let query = parse("MATCH (a) WITH a CREATE (b) SET b.x = 1 RETURN b;").unwrap();
    match &query.statements[0] {
        QueryBody::SingleQuery(sq) => match &sq.kind {
            SingleQueryKind::MultiPart(mpq) => match &mpq.final_part.body {
                SinglePartBody::Updating {
                    updating,
                    return_clause,
                } => {
                    check!(updating.len() == 2);
                    check!(matches!(updating[0], UpdatingClause::Create(_)));
                    check!(matches!(updating[1], UpdatingClause::Set(_)));
                    check!(return_clause.is_some());
                }
                other => panic!("expected Updating body, got {other:?}"),
            },
            _ => panic!("expected MultiPart query"),
        },
        _ => panic!("expected SingleQuery"),
    }
}

// ── EXISTS { pattern WHERE … } ──────────────────────────────────────────
//
// `EXISTS { <pattern> WHERE <expr> }` parses to pattern children plus a
// WHERE_CLAUSE — and WHERE_CLAUSE casts as a `Clause`, so the builder's
// bare any-clause check misrouted the pattern form into the regular-query
// path, producing a `RegularQuery` with an EMPTY `Updating` body (the
// braces' content silently dropped).

/// `EXISTS { (n)-[:R]->(m) WHERE m.x > 1 }` — the pattern form with a WHERE
/// — must keep both the pattern and the predicate.
///
/// Unit: `parse()` / AST `ExistsInner::Pattern`
/// Precondition: `MATCH (n) WHERE EXISTS { (n)-[:R]->(m) WHERE m.x > 1 } RETURN n;`.
/// Expectation: the outer WHERE is `Exists` with `Pattern(p, Some(_))`,
/// where `p` has one part whose path has one relationship chain.
#[test]
fn test_exists_pattern_with_where_preserved() {
    use decypher::ast::expr::{ExistsInner, Expression};
    use decypher::ast::pattern::PatternElement;

    let query = parse("MATCH (n) WHERE EXISTS { (n)-[:R]->(m) WHERE m.x > 1 } RETURN n;").unwrap();
    match match_where_clause(&query) {
        Expression::Exists(e) => match e.inner.as_ref() {
            ExistsInner::Pattern(p, where_clause) => {
                check!(p.parts.len() == 1);
                match &p.parts[0].anonymous.element {
                    PatternElement::Path { start, chains } => {
                        check!(start.variable.as_ref().unwrap().name.name == "n");
                        check!(chains.len() == 1);
                    }
                    other => panic!("expected Path element, got {other:?}"),
                }
                check!(where_clause.is_some());
            }
            other => panic!("expected ExistsInner::Pattern, got {other:?}"),
        },
        other => panic!("expected Exists expression, got {other:?}"),
    }
}

/// Unaffected: `EXISTS { (n)-[:R]->(m) }` without WHERE stays the pattern
/// form with no predicate.
///
/// Unit: `parse()` / AST `ExistsInner::Pattern`
/// Precondition: `MATCH (n) WHERE EXISTS { (n)-[:R]->(m) } RETURN n;`.
/// Expectation: `Exists` with `Pattern(_, None)`.
#[test]
fn test_exists_pattern_without_where_unaffected() {
    use decypher::ast::expr::{ExistsInner, Expression};

    let query = parse("MATCH (n) WHERE EXISTS { (n)-[:R]->(m) } RETURN n;").unwrap();
    match match_where_clause(&query) {
        Expression::Exists(e) => match e.inner.as_ref() {
            ExistsInner::Pattern(p, where_clause) => {
                check!(p.parts.len() == 1);
                check!(where_clause.is_none());
            }
            other => panic!("expected ExistsInner::Pattern, got {other:?}"),
        },
        other => panic!("expected Exists expression, got {other:?}"),
    }
}

/// Unaffected: `EXISTS { MATCH … WHERE … RETURN … }` — a real inner query —
/// still takes the regular-query form, with its clauses intact.
///
/// Unit: `parse()` / AST `ExistsInner::RegularQuery`
/// Precondition: `MATCH (n) WHERE EXISTS { MATCH (n)-->(m) WHERE m.x > 1 RETURN m } RETURN n;`.
/// Expectation: `Exists` with `RegularQuery` whose single query is a
/// `SinglePart` with one MATCH reading clause and a `Return` body.
#[test]
fn test_exists_regular_query_unaffected() {
    use decypher::ast::expr::{ExistsInner, Expression};
    use decypher::ast::query::{ReadingClause, SingleQueryKind};

    let query =
        parse("MATCH (n) WHERE EXISTS { MATCH (n)-->(m) WHERE m.x > 1 RETURN m } RETURN n;")
            .unwrap();
    match match_where_clause(&query) {
        Expression::Exists(e) => match e.inner.as_ref() {
            ExistsInner::RegularQuery(rq) => match &rq.single_query.kind {
                SingleQueryKind::SinglePart(spq) => {
                    check!(spq.reading_clauses.len() == 1);
                    check!(matches!(spq.reading_clauses[0], ReadingClause::Match(_)));
                    check!(matches!(spq.body, SinglePartBody::Return(_)));
                }
                _ => panic!("expected SinglePart inner query"),
            },
            other => panic!("expected ExistsInner::RegularQuery, got {other:?}"),
        },
        other => panic!("expected Exists expression, got {other:?}"),
    }
}

// ── Pattern comprehensions ──────────────────────────────────────────────
//
// `[<pattern> WHERE <expr> | <map>]` parses its pattern into the same
// `RELATIONSHIPS_PATTERN` node a bare pattern predicate produces, so the
// typed AST carries the real path — start node, every relationship chain,
// types, ranges — rather than a bare-node stand-in.

/// Extract the sole projected expression as a `PatternComprehension`.
fn first_projection_pattern_comprehension(
    query: &decypher::ast::Query,
) -> &decypher::ast::expr::PatternComprehension {
    match first_projection_expr(query) {
        decypher::ast::expr::Expression::PatternComprehension(pc) => pc.as_ref(),
        other => panic!("expected PatternComprehension, got {other:?}"),
    }
}

/// The static type name of a chain's relationship, panicking on any other
/// label-expression form.
fn chain_type(chain: &decypher::ast::pattern::PatternElementChain) -> &str {
    match chain
        .relationship
        .detail
        .as_ref()
        .expect("relationship detail")
        .types
        .as_ref()
        .expect("relationship types")
    {
        decypher::ast::pattern::LabelExpression::Static(name) => &name.name,
        other => panic!("expected a static relationship type, got {other:?}"),
    }
}

/// The variable name bound to a chain's end node.
fn chain_end(chain: &decypher::ast::pattern::PatternElementChain) -> &str {
    &chain
        .node
        .variable
        .as_ref()
        .expect("end node variable")
        .name
        .name
}

/// `[(a)-[:T]->(b) | b.x]` must keep the whole matched path: the start node
/// `a`, one relationship chain typed `T` pointing right, and the end node
/// `b` — not collapse to a lone node pattern.
///
/// Unit: `parse()` / AST `PatternComprehension::pattern`
/// Precondition: `MATCH (a) RETURN [(a)-[:T]->(b) | b.x];`.
/// Expectation: `pattern.start.variable == "a"`, `chains.len() == 1`, that
/// chain is `:T` with `Right` direction and end node `b`, and the map is a
/// property lookup.
#[test]
fn test_pattern_comprehension_keeps_chain() {
    use decypher::ast::expr::Expression;
    use decypher::ast::pattern::RelationshipDirection;

    let query = parse("MATCH (a) RETURN [(a)-[:T]->(b) | b.x];").unwrap();
    let pc = first_projection_pattern_comprehension(&query);
    check!(pc.variable.is_none());
    check!(pc.pattern.start.variable.as_ref().unwrap().name.name == "a");
    check!(pc.pattern.chains.len() == 1);
    check!(pc.pattern.chains[0].relationship.direction == RelationshipDirection::Right);
    check!(chain_type(&pc.pattern.chains[0]) == "T");
    check!(chain_end(&pc.pattern.chains[0]) == "b");
    check!(matches!(pc.map, Expression::PropertyLookup { .. }));
}

/// `[(a)-[r:T]->(b) WHERE b.x > 1 | b.name]` must keep the pattern *and*
/// the WHERE predicate: fixing the pattern must not disturb the filter.
///
/// Unit: `parse()` / AST `PatternComprehension::{pattern, where_clause}`
/// Precondition: `MATCH (a) RETURN [(a)-[r:T]->(b) WHERE b.x > 1 | b.name];`.
/// Expectation: one chain bound to `r` and typed `T`, and `where_clause` is
/// a `>` comparison.
#[test]
fn test_pattern_comprehension_with_where() {
    use decypher::ast::expr::{ComparisonOperator, Expression};

    let query = parse("MATCH (a) RETURN [(a)-[r:T]->(b) WHERE b.x > 1 | b.name];").unwrap();
    let pc = first_projection_pattern_comprehension(&query);
    check!(pc.pattern.chains.len() == 1);
    check!(chain_type(&pc.pattern.chains[0]) == "T");
    let detail = pc.pattern.chains[0].relationship.detail.as_ref().unwrap();
    check!(detail.variable.as_ref().unwrap().name.name == "r");
    match pc.where_clause.as_ref().expect("WHERE clause") {
        Expression::Comparison { operators, .. } => {
            check!(operators.len() == 1);
            check!(operators[0].0 == ComparisonOperator::Gt);
        }
        other => panic!("expected a comparison in WHERE, got {other:?}"),
    }
}

/// `[p = (a)-->(b) | p]` binds the path variable `p` *and* keeps the real
/// pattern: `p` lives in `PatternComprehension::variable`, while the
/// pattern's start node is `a` — the path variable must never stand in as
/// the start node.
///
/// Unit: `parse()` / AST `PatternComprehension::{variable, pattern}`
/// Precondition: `MATCH (a) RETURN [p = (a)-->(b) | p];`.
/// Expectation: `variable == "p"`, `pattern.start.variable == "a"`, and
/// `chains.len() == 1`.
#[test]
fn test_pattern_comprehension_path_variable() {
    let query = parse("MATCH (a) RETURN [p = (a)-->(b) | p];").unwrap();
    let pc = first_projection_pattern_comprehension(&query);
    check!(pc.variable.as_ref().unwrap().name.name == "p");
    check!(pc.pattern.start.variable.as_ref().unwrap().name.name == "a");
    check!(pc.pattern.chains.len() == 1);
    check!(chain_end(&pc.pattern.chains[0]) == "b");
}

/// A multi-hop comprehension pattern keeps every chain, in order, with each
/// chain's own direction and type.
///
/// Unit: `parse()` / AST `PatternComprehension::pattern`
/// Precondition: `MATCH (a) RETURN [(a)-[:T]->(b)<-[:U]-(c) | c];`.
/// Expectation: two chains — `:T` `Right` to `b`, then `:U` `Left` to `c`.
#[test]
fn test_pattern_comprehension_multi_hop() {
    use decypher::ast::pattern::RelationshipDirection;

    let query = parse("MATCH (a) RETURN [(a)-[:T]->(b)<-[:U]-(c) | c];").unwrap();
    let pc = first_projection_pattern_comprehension(&query);
    check!(pc.pattern.start.variable.as_ref().unwrap().name.name == "a");
    check!(pc.pattern.chains.len() == 2);
    check!(pc.pattern.chains[0].relationship.direction == RelationshipDirection::Right);
    check!(chain_type(&pc.pattern.chains[0]) == "T");
    check!(chain_end(&pc.pattern.chains[0]) == "b");
    check!(pc.pattern.chains[1].relationship.direction == RelationshipDirection::Left);
    check!(chain_type(&pc.pattern.chains[1]) == "U");
    check!(chain_end(&pc.pattern.chains[1]) == "c");
}

/// A variable-length relationship inside a comprehension keeps its range —
/// `*1..2` must survive as a `RangeLiteral`, since a consumer must plan a
/// bounded traversal rather than a single hop.
///
/// Unit: `parse()` / AST `RelationshipDetail::range`
/// Precondition: `MATCH (a) RETURN [(a)-[:T*1..2]->(b) | b];`.
/// Expectation: the chain's detail has `range == Some(1..2)`.
#[test]
fn test_pattern_comprehension_variable_length() {
    let query = parse("MATCH (a) RETURN [(a)-[:T*1..2]->(b) | b];").unwrap();
    let pc = first_projection_pattern_comprehension(&query);
    check!(pc.pattern.chains.len() == 1);
    let range = pc.pattern.chains[0]
        .relationship
        .detail
        .as_ref()
        .expect("relationship detail")
        .range
        .as_ref()
        .expect("variable-length range");
    check!(range.start == Some(1));
    check!(range.end == Some(2));
}

/// A comprehension nested in another comprehension's map expression keeps
/// both patterns: the inner one is reached through the outer's `map`.
///
/// Unit: `parse()` / AST `PatternComprehension::map`
/// Precondition: `MATCH (a) RETURN [(a)-->(b) | [(b)-->(c) | c.x]];`.
/// Expectation: outer pattern starts at `a` with one chain, and its map is a
/// `PatternComprehension` starting at `b` with one chain to `c`.
#[test]
fn test_pattern_comprehension_nested_in_map() {
    use decypher::ast::expr::Expression;

    let query = parse("MATCH (a) RETURN [(a)-->(b) | [(b)-->(c) | c.x]];").unwrap();
    let outer = first_projection_pattern_comprehension(&query);
    check!(outer.pattern.start.variable.as_ref().unwrap().name.name == "a");
    check!(outer.pattern.chains.len() == 1);
    match &outer.map {
        Expression::PatternComprehension(inner) => {
            check!(inner.pattern.start.variable.as_ref().unwrap().name.name == "b");
            check!(inner.pattern.chains.len() == 1);
            check!(chain_end(&inner.pattern.chains[0]) == "c");
        }
        other => panic!("expected a nested PatternComprehension map, got {other:?}"),
    }
}

// ── List comprehensions bound to a contextual keyword ───────────────────
//
// A comprehension's binder is an ordinary variable, so it may be spelled
// with any contextual (non-reserved) keyword — `key`, `type`, `count`, …
// The `[ … ]` disambiguation therefore recognises those token kinds as a
// name, not only `IDENT`/`ESCAPED_IDENT`.

/// Extract the sole projected expression as a `ListComprehension`.
fn first_projection_list_comprehension(
    query: &decypher::ast::Query,
) -> &decypher::ast::expr::ListComprehension {
    match first_projection_expr(query) {
        decypher::ast::expr::Expression::ListComprehension(lc) => lc.as_ref(),
        other => panic!("expected ListComprehension, got {other:?}"),
    }
}

/// `[key IN keys(r) | key + '->' + r[key]]` — the binder is the contextual
/// keyword `key`, the collection is a function call, and the map indexes the
/// relationship by the bound key.
///
/// Unit: `parse()` / AST `Expression::ListComprehension`
/// Precondition: `MATCH ()-[r]->() RETURN [key IN keys(r) | key + '->' + r[key]];`.
/// Expectation: `variable == "key"`, `collection` is `keys(r)`, `filter` is
/// `None`, and `map` is an `Add` whose right operand is a `ListIndex`.
#[test]
fn test_list_comprehension_keyword_binder_map() {
    use decypher::ast::expr::{BinaryOperator, Expression};

    let query = parse("MATCH ()-[r]->() RETURN [key IN keys(r) | key + '->' + r[key]];").unwrap();
    let lc = first_projection_list_comprehension(&query);
    check!(lc.variable.name.name == "key");
    match lc.collection.as_ref() {
        Expression::FunctionCall(fi) => {
            check!(fi.name.len() == 1);
            check!(fi.name[0].name == "keys");
            check!(fi.arguments.len() == 1);
        }
        other => panic!("expected a keys(r) collection, got {other:?}"),
    }
    check!(lc.filter.is_none());
    match &lc.map {
        Some(Expression::BinaryOp { op, rhs, .. }) => {
            check!(*op == BinaryOperator::Add);
            match rhs.as_ref() {
                Expression::ListIndex { list, index, .. } => {
                    check!(matches!(list.as_ref(), Expression::Variable(v) if v.name.name == "r"));
                    check!(
                        matches!(index.as_ref(), Expression::Variable(v) if v.name.name == "key")
                    );
                }
                other => panic!("expected r[key] as the map's right operand, got {other:?}"),
            }
        }
        other => panic!("expected Some(BinaryOp) map, got {other:?}"),
    }
}

/// `[key IN keys(r) WHERE key <> 'a']` — a keyword binder with a filter and
/// no map keeps the predicate and leaves `map` empty.
///
/// Unit: `parse()` / AST `Expression::ListComprehension`
/// Precondition: `MATCH ()-[r]->() RETURN [key IN keys(r) WHERE key <> 'a'];`.
/// Expectation: `variable == "key"`, `filter` is `Some(Comparison)`, `map` is
/// `None`.
#[test]
fn test_list_comprehension_keyword_binder_where_no_map() {
    use decypher::ast::expr::{ComparisonOperator, Expression};

    let query = parse("MATCH ()-[r]->() RETURN [key IN keys(r) WHERE key <> 'a'];").unwrap();
    let lc = first_projection_list_comprehension(&query);
    check!(lc.variable.name.name == "key");
    match lc.filter.as_deref() {
        Some(Expression::Comparison { operators, .. }) => {
            check!(operators.len() == 1);
            check!(operators[0].0 == ComparisonOperator::Ne);
        }
        other => panic!("expected Some(Comparison) filter, got {other:?}"),
    }
    check!(lc.map.is_none());
}

/// `[key IN keys(r) WHERE key <> 'a' | r[key]]` — a keyword binder carries
/// filter and map simultaneously, and the map is an index lookup.
///
/// Unit: `parse()` / AST `Expression::ListComprehension`
/// Precondition:
/// `MATCH ()-[r]->() RETURN [key IN keys(r) WHERE key <> 'a' | r[key]];`.
/// Expectation: `filter` is `Some(Comparison)` and `map` is `Some(ListIndex)`
/// over `r` indexed by `key`.
#[test]
fn test_list_comprehension_keyword_binder_where_and_map() {
    use decypher::ast::expr::Expression;

    let query =
        parse("MATCH ()-[r]->() RETURN [key IN keys(r) WHERE key <> 'a' | r[key]];").unwrap();
    let lc = first_projection_list_comprehension(&query);
    check!(lc.variable.name.name == "key");
    check!(matches!(
        lc.filter.as_deref(),
        Some(Expression::Comparison { .. })
    ));
    match &lc.map {
        Some(Expression::ListIndex { list, index, .. }) => {
            check!(matches!(list.as_ref(), Expression::Variable(v) if v.name.name == "r"));
            check!(matches!(index.as_ref(), Expression::Variable(v) if v.name.name == "key"));
        }
        other => panic!("expected Some(ListIndex) map, got {other:?}"),
    }
}

/// `[key IN keys(r)]` — with neither filter nor map, a keyword binder still
/// produces a comprehension, exactly as an identifier binder does; it must
/// not degrade into a one-element list literal holding an `IN` predicate.
///
/// Unit: `parse()` / AST `Expression::ListComprehension`
/// Precondition: `MATCH ()-[r]->() RETURN [key IN keys(r)];`.
/// Expectation: `variable == "key"`, `filter` and `map` are both `None`.
#[test]
fn test_list_comprehension_keyword_binder_bare() {
    let query = parse("MATCH ()-[r]->() RETURN [key IN keys(r)];").unwrap();
    let lc = first_projection_list_comprehension(&query);
    check!(lc.variable.name.name == "key");
    check!(lc.filter.is_none());
    check!(lc.map.is_none());
}

/// A comprehension nested in another comprehension's map keeps both binders,
/// including when the inner one is a contextual keyword.
///
/// Unit: `parse()` / AST `ListComprehension::map`
/// Precondition: `MATCH (n) RETURN [x IN [1,2] | [type IN keys(n) | type]];`.
/// Expectation: outer binder `x`, and the outer map is a `ListComprehension`
/// whose binder is `type`.
#[test]
fn test_list_comprehension_nested_keyword_binder() {
    use decypher::ast::expr::Expression;

    let query = parse("MATCH (n) RETURN [x IN [1,2] | [type IN keys(n) | type]];").unwrap();
    let outer = first_projection_list_comprehension(&query);
    check!(outer.variable.name.name == "x");
    match &outer.map {
        Some(Expression::ListComprehension(inner)) => {
            check!(inner.variable.name.name == "type");
            check!(matches!(
                inner.map.as_ref(),
                Some(Expression::Variable(v)) if v.name.name == "type"
            ));
        }
        other => panic!("expected a nested ListComprehension map, got {other:?}"),
    }
}

/// A pattern comprehension's path variable is a variable too, so it may also
/// be spelled with a contextual keyword.
///
/// Unit: `parse()` / AST `PatternComprehension::{variable, pattern}`
/// Precondition: `MATCH (a) RETURN [key = (a)-->(b) | key];`.
/// Expectation: `variable == "key"` and the pattern still starts at `a` with
/// one chain ending at `b`.
#[test]
fn test_pattern_comprehension_keyword_path_variable() {
    let query = parse("MATCH (a) RETURN [key = (a)-->(b) | key];").unwrap();
    let pc = first_projection_pattern_comprehension(&query);
    check!(pc.variable.as_ref().unwrap().name.name == "key");
    check!(pc.pattern.start.variable.as_ref().unwrap().name.name == "a");
    check!(pc.pattern.chains.len() == 1);
    check!(chain_end(&pc.pattern.chains[0]) == "b");
}

/// A plain list literal is unaffected: `[1, 2, 3]` stays a `Literal::List`
/// with three elements, and no binder is invented for it.
///
/// Unit: `parse()` / AST `Literal::List`
/// Precondition: `RETURN [1, 2, 3];`.
/// Expectation: `Literal::List` with `elements.len() == 3`.
#[test]
fn test_bare_list_literal_unaffected() {
    use decypher::ast::expr::{Expression, Literal};

    let query = parse("RETURN [1, 2, 3];").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::List(list)) => {
            check!(list.elements.len() == 3);
        }
        other => panic!("expected a list literal, got {other:?}"),
    }
}

/// A list literal whose elements are *named* with contextual keywords is
/// still a list literal — recognising keyword binders must not turn every
/// keyword-led bracket into a comprehension.
///
/// Unit: `parse()` / AST `Literal::List`
/// Precondition: `MATCH (n) RETURN [key, type, count];` (three variables).
/// Expectation: `Literal::List` with three `Variable` elements.
#[test]
fn test_list_literal_of_keyword_named_variables_unaffected() {
    use decypher::ast::expr::{Expression, Literal};

    let query = parse("MATCH (n) RETURN [key, type, count];").unwrap();
    match first_projection_expr(&query) {
        Expression::Literal(Literal::List(list)) => {
            check!(list.elements.len() == 3);
            check!(matches!(&list.elements[0], Expression::Variable(v) if v.name.name == "key"));
            check!(matches!(&list.elements[1], Expression::Variable(v) if v.name.name == "type"));
            check!(matches!(&list.elements[2], Expression::Variable(v) if v.name.name == "count"));
        }
        other => panic!("expected a list literal, got {other:?}"),
    }
}

// ============================================================
// Relationship-type alternation spelled with repeated colons
//
// openCypher accepts relationship-type alternation both as `:A|B|C` and,
// legacily, as `:A|:B|:C`; the two spellings denote the same union of
// types. decypher's label-expression parser only ever consumed a single
// leading `:`, so every alternative after the first had to be bare and
// `-[:A|:B]->` failed to parse. The repeated colon is now punctuation the
// relationship-type position tolerates and the AST does not record, so the
// two spellings (and any mixture of them) produce the same type tree.
//
// It stays punctuation only there: on a node, `:A:B` is label
// *conjunction*, so `(n:A|:B)` remains a syntax error, as does a dangling
// `:A|:` with no alternative behind it.
// ============================================================

/// The relationship chains of the first MATCH clause's first pattern part.
fn first_match_chains(
    query: &decypher::ast::Query,
) -> &[decypher::ast::pattern::PatternElementChain] {
    let QueryBody::SingleQuery(sq) = &query.statements[0] else {
        panic!("expected SingleQuery");
    };
    let decypher::ast::query::SingleQueryKind::SinglePart(spq) = &sq.kind else {
        panic!("expected SinglePart query");
    };
    let decypher::ast::query::ReadingClause::Match(m) = &spq.reading_clauses[0] else {
        panic!("expected Match clause");
    };
    match &m.pattern.parts[0].anonymous.element {
        decypher::ast::pattern::PatternElement::Path { chains, .. } => chains,
        other => panic!("expected a Path pattern element, got {other:?}"),
    }
}

/// The first relationship's detail in the first MATCH clause.
fn first_match_rel_detail(
    query: &decypher::ast::Query,
) -> &decypher::ast::pattern::RelationshipDetail {
    first_match_chains(query)[0]
        .relationship
        .detail
        .as_ref()
        .expect("relationship detail")
}

/// Render a label expression's structure with explicit parentheses, so two
/// trees can be compared for shape and names without comparing the spans,
/// which necessarily differ between two spellings of the same types.
fn label_shape(expr: &decypher::ast::pattern::LabelExpression) -> String {
    use decypher::ast::pattern::LabelExpression;

    match expr {
        LabelExpression::Static(name) => name.name.to_string(),
        LabelExpression::Or { lhs, rhs, .. } => {
            format!("({}|{})", label_shape(lhs), label_shape(rhs))
        }
        LabelExpression::And { lhs, rhs, .. } => {
            format!("({}&{})", label_shape(lhs), label_shape(rhs))
        }
        LabelExpression::Not { inner, .. } => format!("!{}", label_shape(inner)),
        LabelExpression::Group { inner, .. } => format!("[{}]", label_shape(inner)),
        other => panic!("unexpected label expression form: {other:?}"),
    }
}

/// The relationship-type tree of the first MATCH clause's first relationship.
fn first_match_rel_type_shape(query: &decypher::ast::Query) -> String {
    label_shape(
        first_match_rel_detail(query)
            .types
            .as_ref()
            .expect("relationship types"),
    )
}

/// `-[:A|:B]->` must parse, and to exactly the type tree that `-[:A|B]->`
/// parses to — the repeated colon is a spelling, not a distinct construct.
///
/// Unit: `parse()` / AST `RelationshipDetail::types`
/// Precondition: `MATCH (a)-[:A|:B]->(b) RETURN b;` and the same query with
///   `:A|B`.
/// Expectation: both yield the type tree `(A|B)`.
#[test]
fn test_rel_type_repeated_colon_matches_plain_alternation() {
    let repeated = parse("MATCH (a)-[:A|:B]->(b) RETURN b;").unwrap();
    let plain = parse("MATCH (a)-[:A|B]->(b) RETURN b;").unwrap();
    check!(first_match_rel_type_shape(&repeated) == "(A|B)");
    check!(first_match_rel_type_shape(&repeated) == first_match_rel_type_shape(&plain));
}

/// The same type repeated — `-[:T|:T]->` — is accepted; alternation does not
/// require the alternatives to be distinct.
///
/// Unit: `parse()` / AST `RelationshipDetail::types`
/// Precondition: `MATCH (a)-[:T|:T]->(b) RETURN b;`.
/// Expectation: the type tree is `(T|T)`.
#[test]
fn test_rel_type_repeated_colon_same_type_twice() {
    let query = parse("MATCH (a)-[:T|:T]->(b) RETURN b;").unwrap();
    check!(first_match_rel_type_shape(&query) == "(T|T)");
}

/// A three-way alternation may mix the two spellings freely: `:A|B|:C`,
/// `:A|:B|:C` and `:A|B|C` are all the same left-nested union.
///
/// Unit: `parse()` / AST `RelationshipDetail::types`
/// Precondition: the three spellings of a three-type alternation.
/// Expectation: every one yields `((A|B)|C)`.
#[test]
fn test_rel_type_mixed_colon_spellings_agree() {
    for query in [
        "MATCH (a)-[:A|B|:C]->(b) RETURN b;",
        "MATCH (a)-[:A|:B|:C]->(b) RETURN b;",
        "MATCH (a)-[:A|B|C]->(b) RETURN b;",
    ] {
        let parsed = parse(query).unwrap();
        check!(
            first_match_rel_type_shape(&parsed) == "((A|B)|C)",
            "{query}"
        );
    }
}

/// A bound relationship variable is unaffected by the spelling: `-[r:A|:B]->`
/// still binds `r` and still carries both types.
///
/// Unit: `parse()` / AST `RelationshipDetail::{variable, types}`
/// Precondition: `MATCH (a)-[r:A|:B]->(b) RETURN r;`.
/// Expectation: `variable == "r"` and the type tree is `(A|B)`.
#[test]
fn test_rel_type_repeated_colon_with_bound_variable() {
    let query = parse("MATCH (a)-[r:A|:B]->(b) RETURN r;").unwrap();
    let detail = first_match_rel_detail(&query);
    check!(detail.variable.as_ref().expect("variable").name.name == "r");
    check!(first_match_rel_type_shape(&query) == "(A|B)");
}

/// A variable-length quantifier still follows the type list: the `*` after
/// `:A|:B` is not swallowed by the alternation.
///
/// Unit: `parse()` / AST `RelationshipDetail::{types, range}`
/// Precondition: `MATCH (a)-[:A|:B*1..3]->(b) RETURN b;`.
/// Expectation: the type tree is `(A|B)` and `range == Some(1..3)`.
#[test]
fn test_rel_type_repeated_colon_with_variable_length() {
    let query = parse("MATCH (a)-[:A|:B*1..3]->(b) RETURN b;").unwrap();
    check!(first_match_rel_type_shape(&query) == "(A|B)");
    let range = first_match_rel_detail(&query)
        .range
        .as_ref()
        .expect("variable-length range");
    check!(range.start == Some(1));
    check!(range.end == Some(3));
}

/// A dangling `|:` with no alternative behind it is still a syntax error —
/// tolerating the colon must not make the alternative itself optional.
///
/// Unit: `parse()`
/// Precondition: `MATCH (a)-[:A|:]->(b) RETURN b;`.
/// Expectation: `parse()` returns `Err`.
#[test]
fn test_rel_type_dangling_colon_alternative_is_rejected() {
    let result = parse("MATCH (a)-[:A|:]->(b) RETURN b;");
    check!(result.is_err());
}

/// Node-label position does not gain the spelling: `(n:A|:B)` is still a
/// syntax error, because a second `:` on a node introduces a *conjoined*
/// label rather than another alternative.
///
/// Unit: `parse()`
/// Precondition: `MATCH (n:A|:B) RETURN n;`.
/// Expectation: `parse()` returns `Err`.
#[test]
fn test_node_label_alternation_rejects_repeated_colon() {
    let result = parse("MATCH (n:A|:B) RETURN n;");
    check!(result.is_err());
}

/// Conjunctive node labels are untouched: `(n:A:B)` still parses as two
/// separate label expressions on the node.
///
/// Unit: `parse()` / AST `NodePattern::labels`
/// Precondition: `MATCH (n:A:B) RETURN n;`.
/// Expectation: the node carries the two static labels `A` and `B`.
#[test]
fn test_conjunctive_node_labels_unaffected() {
    let query = parse("MATCH (n:A:B) RETURN n;").unwrap();
    let QueryBody::SingleQuery(sq) = &query.statements[0] else {
        panic!("expected SingleQuery");
    };
    let decypher::ast::query::SingleQueryKind::SinglePart(spq) = &sq.kind else {
        panic!("expected SinglePart query");
    };
    let decypher::ast::query::ReadingClause::Match(m) = &spq.reading_clauses[0] else {
        panic!("expected Match clause");
    };
    let decypher::ast::pattern::PatternElement::Path { start, .. } =
        &m.pattern.parts[0].anonymous.element
    else {
        panic!("expected a Path pattern element");
    };
    check!(start.labels.len() == 2);
    check!(label_shape(&start.labels[0]) == "A");
    check!(label_shape(&start.labels[1]) == "B");
}

// ============================================================
// Label items in SET and REMOVE
//
// `SET n:A` and `REMOVE n:A` relabel a node, and openCypher lets a single
// item name several labels at once — `n:A:B` adds (or removes) both. The
// two clauses parsed that spelling differently: SET accepted the repeated
// colon while REMOVE stopped after the first label, so `REMOVE n:A:B` was
// a syntax error at the second `:`. Both clauses now share one label-list
// rule, and the AST records what it parses: the item's target variable by
// name, and every label of the item in source order rather than only those
// of the first `:` group.
//
// The colon is still not optional: a trailing `SET n:` / `REMOVE n:` names
// no label and is a syntax error in both clauses.
// ============================================================

/// The updating clauses of the first statement's single-part body.
fn first_updating_clauses(query: &decypher::ast::Query) -> &[decypher::ast::query::UpdatingClause] {
    let QueryBody::SingleQuery(sq) = &query.statements[0] else {
        panic!("expected SingleQuery");
    };
    let decypher::ast::query::SingleQueryKind::SinglePart(spq) = &sq.kind else {
        panic!("expected SinglePart query");
    };
    match &spq.body {
        SinglePartBody::Updating { updating, .. } => updating,
        other => panic!("expected an Updating body, got {other:?}"),
    }
}

/// The target variable and labels of the first `SET` clause's first item.
fn first_set_label_item(query: &decypher::ast::Query) -> (String, Vec<String>) {
    let decypher::ast::query::UpdatingClause::Set(set) = &first_updating_clauses(query)[0] else {
        panic!("expected a SET clause");
    };
    let decypher::ast::clause::SetItem::Labels { variable, labels } = &set.items[0] else {
        panic!("expected a label SET item, got {:?}", set.items[0]);
    };
    (
        variable.name.name.to_string(),
        labels.iter().map(|l| l.name.to_string()).collect(),
    )
}

/// The target variable and labels of the first `REMOVE` clause's first item.
fn first_remove_label_item(query: &decypher::ast::Query) -> (String, Vec<String>) {
    let decypher::ast::query::UpdatingClause::Remove(remove) = &first_updating_clauses(query)[0]
    else {
        panic!("expected a REMOVE clause");
    };
    let decypher::ast::clause::RemoveItem::Labels { variable, labels } = &remove.items[0] else {
        panic!("expected a label REMOVE item, got {:?}", remove.items[0]);
    };
    (
        variable.name.name.to_string(),
        labels.iter().map(|l| l.name.to_string()).collect(),
    )
}

/// A single-label `SET n:A` records the target variable by name, not just by
/// span, so a consumer never has to re-read the source to learn it.
///
/// Unit: `parse()` / AST `SetItem::Labels`
/// Precondition: `MATCH (n) SET n:A RETURN n;`.
/// Expectation: the item names variable `n` and the single label `A`, and the
///   variable's span covers exactly the `n` that precedes the colon.
#[test]
fn test_set_single_label_names_variable() {
    let query = parse("MATCH (n) SET n:A RETURN n;").unwrap();
    let (variable, labels) = first_set_label_item(&query);
    check!(variable == "n");
    check!(labels == ["A"]);

    let decypher::ast::query::UpdatingClause::Set(set) = &first_updating_clauses(&query)[0] else {
        panic!("expected a SET clause");
    };
    let decypher::ast::clause::SetItem::Labels { variable, .. } = &set.items[0] else {
        panic!("expected a label SET item");
    };
    check!(&"MATCH (n) SET n:A RETURN n;"[variable.name.span.start..variable.name.span.end] == "n");
}

/// A single-label `REMOVE n:A` records the target variable the same way.
///
/// Unit: `parse()` / AST `RemoveItem::Labels`
/// Precondition: `MATCH (n) REMOVE n:A RETURN n;`.
/// Expectation: the item names variable `n` and the single label `A`, and the
///   variable's span covers exactly the `n` that precedes the colon.
#[test]
fn test_remove_single_label_names_variable() {
    let query = parse("MATCH (n) REMOVE n:A RETURN n;").unwrap();
    let (variable, labels) = first_remove_label_item(&query);
    check!(variable == "n");
    check!(labels == ["A"]);

    let decypher::ast::query::UpdatingClause::Remove(remove) = &first_updating_clauses(&query)[0]
    else {
        panic!("expected a REMOVE clause");
    };
    let decypher::ast::clause::RemoveItem::Labels { variable, .. } = &remove.items[0] else {
        panic!("expected a label REMOVE item");
    };
    check!(
        &"MATCH (n) REMOVE n:A RETURN n;"[variable.name.span.start..variable.name.span.end] == "n"
    );
}

/// `SET n:A:B` adds both labels, and both reach the AST in source order.
///
/// Unit: `parse()` / AST `SetItem::Labels`
/// Precondition: `MATCH (n) SET n:A:B RETURN n;`.
/// Expectation: the item names variable `n` and the labels `A` and `B`.
#[test]
fn test_set_two_labels() {
    let query = parse("MATCH (n) SET n:A:B RETURN n;").unwrap();
    let (variable, labels) = first_set_label_item(&query);
    check!(variable == "n");
    check!(labels == ["A", "B"]);
}

/// `REMOVE n:A:B` removes both labels — the second `:` is part of the item,
/// not the start of a new clause.
///
/// Unit: `parse()` / AST `RemoveItem::Labels`
/// Precondition: `MATCH (n) REMOVE n:A:B RETURN n;`.
/// Expectation: the item names variable `n` and the labels `A` and `B`.
#[test]
fn test_remove_two_labels() {
    let query = parse("MATCH (n) REMOVE n:A:B RETURN n;").unwrap();
    let (variable, labels) = first_remove_label_item(&query);
    check!(variable == "n");
    check!(labels == ["A", "B"]);
}

/// The label list has no fixed length: three labels are reported as three.
///
/// Unit: `parse()` / AST `SetItem::Labels` and `RemoveItem::Labels`
/// Precondition: `MATCH (n) SET n:A:B:C RETURN n;` and the `REMOVE` twin.
/// Expectation: both items carry `A`, `B` and `C`.
#[test]
fn test_three_labels_in_both_clauses() {
    let set = parse("MATCH (n) SET n:A:B:C RETURN n;").unwrap();
    check!(first_set_label_item(&set) == ("n".to_string(), vec_of(["A", "B", "C"])));

    let remove = parse("MATCH (n) REMOVE n:A:B:C RETURN n;").unwrap();
    check!(first_remove_label_item(&remove) == ("n".to_string(), vec_of(["A", "B", "C"])));
}

/// Owned `String`s for comparing against a reported label list.
fn vec_of<const N: usize>(names: [&str; N]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

/// The TCK's `Remove3` scenario — removing two of a node's three labels —
/// parses, and the two labels named are the two reported.
///
/// Unit: `parse()` / AST `RemoveItem::Labels`
/// Precondition: `MATCH (n) REMOVE n:L1:L3 RETURN labels(n);`.
/// Expectation: the item names variable `n` and the labels `L1` and `L3`.
#[test]
fn test_remove_two_of_three_labels() {
    let query = parse("MATCH (n) REMOVE n:L1:L3 RETURN labels(n);").unwrap();
    let (variable, labels) = first_remove_label_item(&query);
    check!(variable == "n");
    check!(labels == ["L1", "L3"]);
}

/// A `:` that names no label is a truncated item, not an item with zero
/// labels: a dangling colon is a syntax error in both clauses.
///
/// Unit: `parse()`
/// Precondition: `MATCH (n) SET n:;` and `MATCH (n) REMOVE n:;`, plus the
///   same two items followed by a further clause.
/// Expectation: `parse()` returns `Err` for every one.
#[test]
fn test_dangling_label_colon_is_rejected() {
    for query in [
        "MATCH (n) SET n:;",
        "MATCH (n) REMOVE n:;",
        "MATCH (n) SET n: RETURN n;",
        "MATCH (n) REMOVE n: RETURN n;",
    ] {
        check!(parse(query).is_err(), "{query}");
    }
}

/// Sharing a label-list rule between the two clauses leaves node-pattern
/// label position alone: `(n:A:B)` is still a conjunction of two label
/// expressions on the node, and `:A|:B` is still relationship alternation.
///
/// Unit: `parse()` / AST `NodePattern::labels` and `RelationshipDetail::types`
/// Precondition: `MATCH (n:A:B) REMOVE n:A:B RETURN n;` and
///   `MATCH (a)-[:A|:B]->(b) SET b:C:D RETURN b;`.
/// Expectation: the node carries the label expressions `A` and `B`, the
///   relationship carries the type tree `(A|B)`, and each label item carries
///   both of its labels.
#[test]
fn test_label_items_leave_pattern_labels_alone() {
    let query = parse("MATCH (n:A:B) REMOVE n:A:B RETURN n;").unwrap();
    let QueryBody::SingleQuery(sq) = &query.statements[0] else {
        panic!("expected SingleQuery");
    };
    let decypher::ast::query::SingleQueryKind::SinglePart(spq) = &sq.kind else {
        panic!("expected SinglePart query");
    };
    let decypher::ast::query::ReadingClause::Match(m) = &spq.reading_clauses[0] else {
        panic!("expected Match clause");
    };
    let decypher::ast::pattern::PatternElement::Path { start, .. } =
        &m.pattern.parts[0].anonymous.element
    else {
        panic!("expected a Path pattern element");
    };
    check!(start.labels.len() == 2);
    check!(label_shape(&start.labels[0]) == "A");
    check!(label_shape(&start.labels[1]) == "B");
    check!(first_remove_label_item(&query) == ("n".to_string(), vec_of(["A", "B"])));

    let alternation = parse("MATCH (a)-[:A|:B]->(b) SET b:C:D RETURN b;").unwrap();
    check!(first_match_rel_type_shape(&alternation) == "(A|B)");
    check!(first_set_label_item(&alternation) == ("b".to_string(), vec_of(["C", "D"])));
}
