use crate::front::lexer::Token;
use lalrpop_util::ParseError;

pub fn format_parse_error(
    source: &str,
    file_path: &str,
    error: ParseError<usize, Token, String>,
) -> String {
    match error {
        ParseError::InvalidToken { location } => {
            let (line, col) = get_line_col(source, location);
            let snippet = get_snippet(source, line);
            let pointer = "".repeat(col.saturating_add(1)) + "^";
            format!(
                "Error in {}:{}:{}: InvalidToken\n{}\n{}",
                file_path, line, col, snippet, pointer
            )
        }
        ParseError::UnrecognizedToken {
            token: (start, token, _end),
            expected,
        } => {
            let (line, col) = get_line_col(source, start);
            let snippet = get_snippet(source, line);
            let pointer = "".repeat(col.saturating_add(1)) + "^";
            format!(
                "Error in {}:{}:{}: UnrecognizedToken '{:?}'\n\n{}\n{}\nExpected: {:?}",
                file_path, line, col, token, snippet, pointer, expected
            )
        }
        ParseError::ExtraToken {
            token: (start, token, _end),
        } => {
            let (line, col) = get_line_col(source, start);
            let snippet = get_snippet(source, line);
            let pointer = "".repeat(col.saturating_add(1)) + "^";
            format!(
                "Error in {}:{}:{}: ExtraToken '{:?}'\n\n{}\n{}",
                file_path, line, col, token, snippet, pointer
            )
        }
        ParseError::User { error } => {
            format!("Error in {}: User error: {}", file_path, error)
        }
        ParseError::UnrecognizedEof { location, expected } => {
            let (line, col) = get_line_col(source, location);
            format!(
                "Error in {}:{}:{}: UnrecognizedEOF\nExpected: {:?}",
                file_path, line, col, expected
            )
        }
    }
}

fn get_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, c) in source.char_indices() {
        if i == offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn get_snippet(source: &str, line_number: usize) -> String {
    source
        .lines()
        .nth(line_number - 1)
        .unwrap_or("")
        .to_string()
}
