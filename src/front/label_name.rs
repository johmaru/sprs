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
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '{' => {
                if !lit.is_empty() {
                    parts.push(LabelNamePart::Lit(std::mem::take(&mut lit)));
                }
                i += 1;
                if i >= chars.len() {
                    return Err("unclosed '{' in dynamic label name".to_string());
                }
                if chars[i] == '}' {
                    return Err("empty '{}' is not allowed in dynamic label name".to_string());
                }
                let start = i;
                if !(chars[i].is_ascii_alphabetic() || chars[i] == '_') {
                    return Err(format!(
                        "invalid interpolation in dynamic label name: expected ident, found '{{{}}}'",
                        collect_until_close(&chars, i)
                    ));
                }
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                if i >= chars.len() || chars[i] != '}' {
                    return Err(format!(
                        "invalid interpolation in dynamic label name: expected ident, found '{{{}}}'",
                        collect_until_close(&chars, start)
                    ));
                }
                let ident: String = chars[start..i].iter().collect();
                parts.push(LabelNamePart::Ident(ident));
                i += 1; // skip '}'
            }
            '}' => {
                return Err("unexpected '}' in dynamic label name".to_string());
            }
            c => {
                lit.push(c);
                i += 1;
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
    for &c in &chars[start..] {
        if c == '}' {
            break;
        }
        out.push(c);
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
        let parts = parse_dynamic_label_template("{i}-item").unwrap();
        assert_eq!(
            parts,
            vec![
                LabelNamePart::Ident("i".into()),
                LabelNamePart::Lit("-item".into()),
            ]
        );
    }

    #[test]
    fn parses_prefix_and_suffix() {
        let parts = parse_dynamic_label_template("item-{n}-x").unwrap();
        assert_eq!(
            parts,
            vec![
                LabelNamePart::Lit("item-".into()),
                LabelNamePart::Ident("n".into()),
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
        assert!(parse_dynamic_label_template("{i+1}").is_err());
    }

    #[test]
    fn rejects_unclosed() {
        assert!(parse_dynamic_label_template("{i").is_err());
    }
}
