//! Unit tests for `tests` split out of `symbols.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn go_functions_and_methods_are_named() {
    let src = "func Build(x int) int {\nfunc (g *Graph) Save() error {\n";
    assert_eq!(
        symbols_of("a.go", src),
        ["Build", "Save"].map(String::from).to_vec()
    );
}

#[test]
fn markdown_headings_are_listed_without_their_marks() {
    assert_eq!(
        symbols_of("a.md", "# Title\n## Part\n#### Deep"),
        ["Title", "Part"].map(String::from).to_vec()
    );
}

#[test]
fn dart_type_wins_over_a_method_shape() {
    let lines = ["extension type Foo(int x) {", "void run() {"];
    assert_eq!(dart_line_symbols(&lines, 0), ["Foo"]);
    assert_eq!(dart_line_symbols(&lines, 1), ["run"]);
}

#[test]
fn unknown_extension_has_no_symbols() {
    assert!(symbols_of("a.bin", "class X").is_empty());
}
