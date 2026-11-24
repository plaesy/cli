use super::*;
use serde_json::json;

fn env() -> Env {
    let mut env = Map::new();
    env.insert(
        "flow".to_owned(),
        json!({ "input": { "warehouse": "gudang-1", "qty": 3 } }),
    );
    env.insert(
        "m1".to_owned(),
        json!({ "invoice": { "status": "draft", "total": 10.5 } }),
    );
    env
}

fn run_expr(text: &str) -> Result<Value, String> {
    eval(&parse(text)?, &env())
}

#[test]
fn expr_path_and_literals() {
    assert_eq!(run_expr("flow.input.warehouse").unwrap(), json!("gudang-1"));
    assert_eq!(run_expr("m1.invoice.total").unwrap(), json!(10.5));
    assert_eq!(run_expr("'halo'").unwrap(), json!("halo"));
    assert_eq!(run_expr("true").unwrap(), json!(true));
    assert_eq!(run_expr("null").unwrap(), Value::Null);
}

#[test]
fn expr_comparisons_including_numbers_and_strings() {
    assert_eq!(
        run_expr("m1.invoice.status == 'draft'").unwrap(),
        json!(true)
    );
    assert_eq!(
        run_expr("m1.invoice.status != 'draft'").unwrap(),
        json!(false)
    );
    assert_eq!(run_expr("flow.input.qty >= 3").unwrap(), json!(true));
    assert_eq!(run_expr("flow.input.qty < 3").unwrap(), json!(false));
    assert_eq!(run_expr("1 == 1.0").unwrap(), json!(true));
    assert_eq!(run_expr("'a' < 'b'").unwrap(), json!(true));
}

#[test]
fn expr_precedence_and_before_or() {
    // Dibaca sebagai: true || (false && false) -> true.
    assert_eq!(run_expr("true || false && false").unwrap(), json!(true));
    // Dibaca sebagai: (false || true) && false -> false.
    assert_eq!(run_expr("(false || true) && false").unwrap(), json!(false));
}

#[test]
fn expr_ternary_and_negation() {
    assert_eq!(
        run_expr("flow.input.qty > 1 ? 'banyak' : 'sedikit'").unwrap(),
        json!("banyak")
    );
    assert_eq!(run_expr("!(flow.input.qty == 3)").unwrap(), json!(false));
    assert_eq!(run_expr("-2 < 0").unwrap(), json!(true));
}

#[test]
fn expr_and_or_short_circuit_before_evaluating_the_right_side() {
    // Sisi kanan merujuk nama yang tidak ada, tetapi tidak pernah dievaluasi.
    assert_eq!(run_expr("false && nama_tidak_ada.x").unwrap(), json!(false));
    assert_eq!(run_expr("true || nama_tidak_ada.x").unwrap(), json!(true));
}

#[test]
fn expr_missing_key_is_an_error_not_a_guess() {
    assert!(run_expr("flow.input.tidak_ada")
        .unwrap_err()
        .contains("tidak ada"));
    assert!(run_expr("nama_tidak_ada")
        .unwrap_err()
        .contains("tidak dikenal"));
}

#[test]
fn expr_outside_the_subset_is_refused_at_parse_time() {
    assert!(parse("a + b").is_err());
    assert!(parse("f(x)").is_err());
    assert!(parse("'tanpa tutup").is_err());
    assert!(parse("1 2").is_err());
}

#[test]
fn expr_eval_bool_rejects_non_bool_results() {
    let expr = parse("flow.input.qty").unwrap();
    assert!(eval_bool(&expr, &env()).is_err());
}
