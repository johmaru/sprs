use crate::front::ast;
use crate::front::error::SprsError;
use crate::front::lexer;
use crate::grammar;
use crate::front::error_reporter;

pub fn parse_only(input: &str, file_path: &str) -> Result<Vec<ast::Item>, SprsError> {
    let mut lex = lexer::Lexer::new(input);
    match grammar::StartParser::new().parse(&mut lex) {
        Ok(items) => Ok(items),
        Err(e) => Err(error_reporter::to_sprs_error(input, file_path, e)),
    }
}
