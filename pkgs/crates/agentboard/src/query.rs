use anyhow::{anyhow, bail, Result};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Term(String, String),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}

pub fn parse_query(s: &str) -> Result<Expr> {
    let normalized = s.replace("NOT ", "-");
    let condition = search_query_parser::parse_query_to_condition(&normalized)
        .map_err(|err| anyhow!("invalid query: {err}"))?;
    expr_from_condition(condition)
}

fn expr_from_condition(condition: search_query_parser::Condition) -> Result<Expr> {
    use search_query_parser::{Condition, Operator};

    match condition {
        Condition::None => bail!("query cannot be empty"),
        Condition::Keyword(token) | Condition::PhraseKeyword(token) => term_from_token(&token),
        Condition::Not(inner) => Ok(Expr::Not(Box::new(expr_from_condition(*inner)?))),
        Condition::Operator(Operator::And, items) => fold_conditions(items, Expr::And),
        Condition::Operator(Operator::Or, items) => fold_conditions(items, Expr::Or),
    }
}

fn fold_conditions(
    items: Vec<search_query_parser::Condition>,
    combine: fn(Box<Expr>, Box<Expr>) -> Expr,
) -> Result<Expr> {
    let mut items = items.into_iter().map(expr_from_condition);
    let first = items
        .next()
        .ok_or_else(|| anyhow!("query group cannot be empty"))??;
    items.try_fold(first, |acc, item| {
        Ok(combine(Box::new(acc), Box::new(item?)))
    })
}

fn term_from_token(token: &str) -> Result<Expr> {
    let (field, value) = token
        .split_once(':')
        .ok_or_else(|| anyhow!("fieldless terms are not allowed"))?;
    if field.is_empty() || value.is_empty() {
        bail!("query terms must be field:value");
    }
    Ok(Expr::Term(field.to_string(), value.to_string()))
}

pub fn eval_query(e: &Expr, fm: &Value) -> bool {
    match e {
        Expr::Term(k, v) => fm.get(k).is_some_and(|actual| value_matches(actual, v)),
        Expr::And(a, b) => eval_query(a, fm) && eval_query(b, fm),
        Expr::Or(a, b) => eval_query(a, fm) || eval_query(b, fm),
        Expr::Not(a) => !eval_query(a, fm),
    }
}

fn value_matches(actual: &Value, expected: &str) -> bool {
    match actual {
        Value::String(s) => s == expected,
        Value::Array(items) => items.iter().any(|v| value_matches(v, expected)),
        Value::Number(n) => n.to_string() == expected,
        Value::Bool(b) => b.to_string() == expected,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn query_matches_scalars_and_arrays() {
        let q = parse_query("status:ready AND (priority:high OR labels:agent)").unwrap();
        let fm = json!({"status":"ready", "priority":"low", "labels":["agent"]});
        assert!(eval_query(&q, &fm));
    }

    #[test]
    fn fieldless_query_is_invalid() {
        assert!(parse_query("ready").is_err());
    }
}
