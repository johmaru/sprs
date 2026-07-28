use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::lexer::Token;
use crate::front::span::Span;
use lalrpop_util::ParseError;

/// lalrpop の ParseError を SprsError::Parse に変換
pub fn to_sprs_error(
    source: &str,
    file_path: &str,
    error: ParseError<usize, Token, String>,
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
            token: (start, token, _end),
            expected,
        } => {
            let span = Span::new(start, start);
            let expected_strs: Vec<String> = expected.iter().map(|e| format!("{:?}", e)).collect();
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
            token: (start, token, _end),
        } => SprsError::Parse {
            code: ErrorCode {
                category: ErrorCategory::Syntax,
                number: 3,
            },
            location: Location::new(file_path.to_string(), Span::new(start, start)),
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
                message: "UnrecognizedEOF".to_string(),
                expected: expected_strs,
                help: None,
            }
        }
        ParseError::User { error } => {
            // User error はメッセージ内容でコード判定
            let code = if error.contains("Invalid assignment target") {
                ErrorCode {
                    category: ErrorCategory::Syntax,
                    number: 5,
                }
            } else if error.contains("Expected IDENT token")
                || error.contains("Expected MACRO token")
                || error.contains("Expected NUM token")
                || error.contains("Expected FLOAT token")
                || error.contains("Expected StrLiteral token")
            {
                ErrorCode {
                    category: ErrorCategory::Syntax,
                    number: 6,
                }
            } else if error.contains("does not support struct init syntax") {
                ErrorCode {
                    category: ErrorCategory::Syntax,
                    number: 7,
                }
            } else {
                ErrorCode {
                    category: ErrorCategory::Syntax,
                    number: 6,
                }
            };
            SprsError::Parse {
                code,
                location: Location::new(file_path.to_string(), Span::DUMMY),
                message: error,
                expected: vec![],
                help: None,
            }
        }
    }
}
