use super::trim_generated_typescript;

#[test]
fn generated_typescript_has_one_terminal_newline() {
    assert_eq!(trim_generated_typescript("export {};\n\n"), "export {};\n");
    assert_eq!(trim_generated_typescript("export {};"), "export {};\n");
}
