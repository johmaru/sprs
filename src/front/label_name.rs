/// Static/dynamic label-name AST pieces for Phase 2.
///
/// Dynamic names come from `:"{ident}-lit"` templates. The parser expands them
/// into `Lit` / `Ident` parts so runtime never re-parses the template string.

#[derive(Debug, PartialEq, Clone)]
pub enum LabelName {
    Static(String),
    Dynamic(Vec<LabelNamePart>),
}

#[derive(Debug, PartialEq, Clone)]
pub enum LabelNamePart {
    Lit(String),
    Ident(String),
}

/// Parse a dynamic label template body (the contents of the string after `:`).
///
/// Rules:
/// - Plain characters become `Lit` (adjacent chars are coalesced)
/// - `{ident}` interpolations become `Ident` (`ident` = `[A-Za-z_][A-Za-z0-9_]*`)
/// - `{}`, nested braces, `{expr}`, unclosed `{` are errors
/// - Zero interpolations (e.g. `"ok"`) are still accepted as Dynamic
pub fn parse_dynamic_label_template(input: &str) -> Result<Vec<LabelNamePart>, String> {
    let mut parts: Vec<LabelNamePart> = Vec::new();
    let mut lit = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut cursor_index = 0;

    while cursor_index < chars.len() {
        match chars[cursor_index] {
            '{' => {
                if !lit.is_empty() {
                    parts.push(LabelNamePart::Lit(std::mem::take(&mut lit)));
                }
                cursor_index += 1;
                if cursor_index >= chars.len() {
                    return Err("unclosed '{' in dynamic label name".to_string());
                }
                if chars[cursor_index] == '}' {
                    return Err("empty '{}' is not allowed in dynamic label name".to_string());
                }
                let start = cursor_index;
                if !(chars[cursor_index].is_ascii_alphabetic() || chars[cursor_index] == '_') {
                    return Err(format!(
                        "invalid interpolation in dynamic label name: expected ident, found '{{{}}}'",
                        collect_until_close(&chars, cursor_index)
                    ));
                }
                cursor_index += 1;
                while cursor_index < chars.len() && (chars[cursor_index].is_ascii_alphanumeric() || chars[cursor_index] == '_') {
                    cursor_index += 1;
                }
                if cursor_index >= chars.len() || chars[cursor_index] != '}' {
                    return Err(format!(
                        "invalid interpolation in dynamic label name: expected ident, found '{{{}}}'",
                        collect_until_close(&chars, start)
                    ));
                }
                let ident: String = chars[start..cursor_index].iter().collect();
                parts.push(LabelNamePart::Ident(ident));
                cursor_index += 1; // skip '}'
            }
            '}' => {
                return Err("unexpected '}' in dynamic label name".to_string());
            }
            character => {
                lit.push(character);
                cursor_index += 1;
            }
        }
    }

    if !lit.is_empty() {
        parts.push(LabelNamePart::Lit(lit));
    }
    Ok(parts)
}

fn collect_until_close(chars: &[char], start: usize) -> String {
    let mut out = String::new();
    for &character in &chars[start..] {
        if character == '}' {
            break;
        }
        out.push(character);
        // Cap for error messages
        if out.len() > 32 {
            out.push_str("...");
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ident_with_surrounding_lits() {
        let parts = parse_dynamic_label_template("{item_index}-item").unwrap();
        assert_eq!(
            parts,
            vec![
                LabelNamePart::Ident("item_index".into()),
                LabelNamePart::Lit("-item".into()),
            ]
        );
    }

    #[test]
    fn parses_prefix_and_suffix() {
        let parts = parse_dynamic_label_template("item-{item_number}-x").unwrap();
        assert_eq!(
            parts,
            vec![
                LabelNamePart::Lit("item-".into()),
                LabelNamePart::Ident("item_number".into()),
                LabelNamePart::Lit("-x".into()),
            ]
        );
    }

    #[test]
    fn allows_zero_interpolations() {
        let parts = parse_dynamic_label_template("ok").unwrap();
        assert_eq!(parts, vec![LabelNamePart::Lit("ok".into())]);
    }

    #[test]
    fn rejects_empty_braces() {
        assert!(parse_dynamic_label_template("{}").is_err());
    }

    #[test]
    fn rejects_expr_interpolation() {
        assert!(parse_dynamic_label_template("{item_index+1}").is_err());
    }

    #[test]
    fn rejects_unclosed() {
        assert!(parse_dynamic_label_template("{i").is_err());
    }
}
