use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::lexer::Token;
use crate::front::parse_error::ParserUserError;
use crate::front::span::Span;
use lalrpop_util::ParseError;

/// Convert lalrpop's ParseError into SprsError.
pub fn to_sprs_error(
    _source: &str,
    file_path: &str,
    error: ParseError<usize, Token, ParserUserError>,
) -> SprsError {
    match error {
        ParseError::InvalidToken { location } => SprsError::Parse {
            code: ErrorCode {
                category: ErrorCategory::Syntax,
                number: 1,
            },
            location: Location::new(file_path.to_string(), Span::new(location, location)),
            message: "InvalidToken".to_string(),
            expected: vec![],
            help: None,
        },
        ParseError::UnrecognizedToken {
            token: (start, token, end),
            expected,
        } => {
            let span = Span::new(start, end);
            let expected_strs: Vec<String> = expected.iter().map(|e| format!("{:?}", e)).collect();
            let wants_ident = expected.iter().any(|e| {
                let text = e.to_ascii_lowercase();
                text.contains("ident") || text.contains("escaped")
            });
            if wants_ident {
                if let Some(keyword) = token.keyword_name() {
                    return SprsError::Parse {
                        code: ErrorCode {
                            category: ErrorCategory::Syntax,
                            number: 2,
                        },
                        location: Location::new(file_path.to_string(), span),
                        message: format!("`{keyword}` is a reserved keyword"),
                        expected: expected_strs,
                        help: Some(format!(
                            "use ^{keyword} if this name is intentional"
                        )),
                    };
                }
            }
            SprsError::Parse {
                code: ErrorCode {
                    category: ErrorCategory::Syntax,
                    number: 2,
                },
                location: Location::new(file_path.to_string(), span),
                message: format!("UnrecognizedToken '{:?}'", token),
                expected: expected_strs,
                help: None,
            }
        }
        ParseError::ExtraToken {
            token: (start, token, end),
        } => SprsError::Parse {
            code: ErrorCode {
                category: ErrorCategory::Syntax,
                number: 3,
            },
            location: Location::new(file_path.to_string(), Span::new(start, end)),
            message: format!("ExtraToken '{:?}'", token),
            expected: vec![],
            help: None,
        },
        ParseError::UnrecognizedEof { location, expected } => {
            let expected_strs: Vec<String> = expected.iter().map(|e| format!("{:?}", e)).collect();
            SprsError::Parse {
                code: ErrorCode {
                    category: ErrorCategory::Syntax,
                    number: 4,
                },
                location: Location::new(file_path.to_string(), Span::new(location, location)),
                message: "UnrecognizedEof".to_string(),
                expected: expected_strs,
                help: None,
            }
        }
        ParseError::User { error } => user_to_sprs_error(file_path, error),
    }
}

fn user_to_sprs_error(file_path: &str, error: ParserUserError) -> SprsError {
    let location = Location::new(file_path.to_string(), error.span);
    match error.category {
        ErrorCategory::Syntax => SprsError::Parse {
            code: ErrorCode {
                category: ErrorCategory::Syntax,
                number: error.number,
            },
            location,
            message: error.message,
            expected: vec![],
            help: error.help,
        },
        ErrorCategory::Semantic => SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: error.number,
            },
            location,
            message: error.message,
            help: error.help,
        },
        ErrorCategory::Type => SprsError::Type {
            code: ErrorCode {
                category: ErrorCategory::Type,
                number: error.number,
            },
            location,
            message: error.message,
            expected_type: None,
            actual_type: None,
            help: error.help,
        },
    }
}
