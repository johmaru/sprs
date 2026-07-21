use crate::front::lexer;
use crate::grammar;
use crate::llvm::error_helper;

pub fn parse_only(input: &str, file_path: &str) -> Result<Vec<crate::front::ast::Item>, String> {
    let mut lex = lexer::Lexer::new(input);
    match grammar::StartParser::new().parse(&mut lex) {
        Ok(items) => Ok(items),
        Err(e) => {
            let error_message = error_helper::format_parse_error(input, file_path, e);
            Err(error_message)
        }
    }
}
