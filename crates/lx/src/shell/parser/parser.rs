//! 语法分析器 (Parser): 将 Token 流转换为 AST
//!
//! 语法规则 (简化版 POSIX shell):
//!
//! ```text
//! script      = command_list (('\n' | ';') command_list)*
//! command_list = pipeline (('&&' | '||') pipeline)*
//! pipeline     = simple_command ('|' simple_command)*
//! simple_command = word (word | redirect)*
//! redirect     = ('>' | '>>' | '<' | '2>') word
//! word         = WORD | SINGLE_QUOTED | DOUBLE_QUOTED | VARIABLE
//! ```

use super::ast::*;
use super::lexer::{LexError, Lexer, Token};

/// 解析错误
#[derive(Debug, Clone)]
pub enum ParseError {
    /// 词法分析错误
    LexError(LexError),
    /// 意外的 token
    UnexpectedToken(String),
    /// 意外的输入结束
    UnexpectedEof,
    /// 空输入
    EmptyInput,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::LexError(e) => write!(f, "syntax error: {e}"),
            ParseError::UnexpectedToken(t) => write!(f, "syntax error near unexpected token `{t}`"),
            ParseError::UnexpectedEof => write!(f, "syntax error: unexpected end of input"),
            ParseError::EmptyInput => write!(f, "no input"),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError::LexError(e)
    }
}

/// 语法分析器
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let token = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// 跳过换行和分号
    fn skip_separators(&mut self) {
        while let Some(token) = self.peek() {
            match token {
                Token::Newline | Token::Semicolon => {
                    self.advance();
                }
                _ => break,
            }
        }
    }

    /// 检查 token 是否是 word-like (可以作为命令名或参数)
    fn is_word_token(token: &Token) -> bool {
        matches!(
            token,
            Token::Word(_) | Token::SingleQuoted(_) | Token::DoubleQuoted(_) | Token::Variable(_)
        )
    }

    /// 检查 token 是否是重定向操作符
    fn is_redirect_token(token: &Token) -> bool {
        matches!(
            token,
            Token::Redirect | Token::Append | Token::RedirectIn | Token::RedirectErr
        )
    }

    /// 将 Token 转换为 Word AST 节点
    fn token_to_word(token: &Token) -> Option<Word> {
        match token {
            Token::Word(s) => {
                // 检查是否包含 glob 字符
                if s.contains('*') || s.contains('?') {
                    Some(Word::Glob(s.clone()))
                } else {
                    Some(Word::Literal(s.clone()))
                }
            }
            Token::SingleQuoted(s) => Some(Word::SingleQuoted(s.clone())),
            Token::DoubleQuoted(s) => {
                // 简化处理: 双引号内如果包含 $ 则解析变量
                if s.contains('$') {
                    let parts = parse_double_quoted_parts(s);
                    Some(Word::DoubleQuoted(parts))
                } else {
                    Some(Word::DoubleQuoted(vec![WordPart::Text(s.clone())]))
                }
            }
            Token::Variable(s) => Some(Word::Variable(s.clone())),
            _ => None,
        }
    }

    /// 解析脚本 (顶层)
    fn parse_script(&mut self) -> Result<Script, ParseError> {
        let mut command_lists = Vec::new();

        self.skip_separators();

        while !self.is_at_end() {
            let list = self.parse_command_list()?;
            command_lists.push(list);
            self.skip_separators();
        }

        if command_lists.is_empty() {
            return Err(ParseError::EmptyInput);
        }

        Ok(Script { command_lists })
    }

    /// 解析命令列表: pipeline (('&&' | '||') pipeline)*
    fn parse_command_list(&mut self) -> Result<CommandList, ParseError> {
        let first = self.parse_pipeline()?;
        let mut rest = Vec::new();

        loop {
            match self.peek() {
                Some(Token::And) => {
                    self.advance();
                    let pipeline = self.parse_pipeline()?;
                    rest.push((ListOp::And, pipeline));
                }
                Some(Token::Or) => {
                    self.advance();
                    let pipeline = self.parse_pipeline()?;
                    rest.push((ListOp::Or, pipeline));
                }
                _ => break,
            }
        }

        Ok(CommandList { first, rest })
    }

    /// 解析管道: `simple_command` ('|' `simple_command`)*
    fn parse_pipeline(&mut self) -> Result<Pipeline, ParseError> {
        let mut commands = Vec::new();
        let first = self.parse_simple_command()?;
        commands.push(first);

        while let Some(Token::Pipe) = self.peek() {
            self.advance();
            let cmd = self.parse_simple_command()?;
            commands.push(cmd);
        }

        Ok(Pipeline { commands })
    }

    /// 解析简单命令: word (word | redirect)*
    fn parse_simple_command(&mut self) -> Result<SimpleCommand, ParseError> {
        // 命令名必须是 word-like token
        let name_token = self.peek().ok_or(ParseError::UnexpectedEof)?;

        if !Self::is_word_token(name_token) {
            return Err(ParseError::UnexpectedToken(format!("{name_token:?}")));
        }

        let name_token = self.advance().unwrap();
        let name = Self::token_to_word(&name_token).unwrap();

        let mut args = Vec::new();
        let mut redirects = Vec::new();

        // 读取参数和重定向
        loop {
            match self.peek() {
                Some(token) if Self::is_word_token(token) => {
                    let token = self.advance().unwrap();
                    let word = Self::token_to_word(&token).unwrap();
                    args.push(word);
                }
                Some(token) if Self::is_redirect_token(token) => {
                    let redirect = self.parse_redirect()?;
                    redirects.push(redirect);
                }
                _ => break,
            }
        }

        Ok(SimpleCommand {
            name,
            args,
            redirects,
        })
    }

    /// 解析重定向: ('>' | '>>' | '<' | '2>') word
    fn parse_redirect(&mut self) -> Result<Redirect, ParseError> {
        let op_token = self.advance().ok_or(ParseError::UnexpectedEof)?;

        let (op, fd) = match op_token {
            Token::Redirect => (RedirectOp::Write, Some(1)),
            Token::Append => (RedirectOp::Append, Some(1)),
            Token::RedirectIn => (RedirectOp::Read, Some(0)),
            Token::RedirectErr => (RedirectOp::WriteStderr, Some(2)),
            _ => return Err(ParseError::UnexpectedToken(format!("{op_token:?}"))),
        };

        // 重定向目标
        let target_token = self.peek().ok_or(ParseError::UnexpectedEof)?;
        if !Self::is_word_token(target_token) {
            return Err(ParseError::UnexpectedToken(format!("{target_token:?}")));
        }

        let target_token = self.advance().unwrap();
        let target = Self::token_to_word(&target_token).unwrap();

        Ok(Redirect { fd, op, target })
    }
}

/// 解析双引号内的变量引用
fn parse_double_quoted_parts(s: &str) -> Vec<WordPart> {
    let mut parts = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut pos = 0;
    let mut current_text = String::new();

    while pos < chars.len() {
        if chars[pos] == '$' {
            // 保存已有文本
            if !current_text.is_empty() {
                parts.push(WordPart::Text(current_text.clone()));
                current_text.clear();
            }

            pos += 1; // skip $

            // 读取变量名
            if pos < chars.len() && chars[pos] == '{' {
                pos += 1; // skip {
                let start = pos;
                while pos < chars.len() && chars[pos] != '}' {
                    pos += 1;
                }
                let var_name: String = chars[start..pos].iter().collect();
                parts.push(WordPart::Variable(var_name));
                if pos < chars.len() {
                    pos += 1; // skip }
                }
            } else {
                let start = pos;
                while pos < chars.len() && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
                    pos += 1;
                }
                let var_name: String = chars[start..pos].iter().collect();
                if !var_name.is_empty() {
                    parts.push(WordPart::Variable(var_name));
                }
            }
        } else {
            current_text.push(chars[pos]);
            pos += 1;
        }
    }

    if !current_text.is_empty() {
        parts.push(WordPart::Text(current_text));
    }

    parts
}

/// 解析入口函数: 字符串 → AST
pub fn parse(input: &str) -> Result<Script, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize()?;

    if tokens.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let mut parser = Parser::new(tokens);
    parser.parse_script()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_command() {
        let script = parse("ls -la /kb").unwrap();
        assert_eq!(script.command_lists.len(), 1);
        let pipeline = &script.command_lists[0].first;
        assert_eq!(pipeline.commands.len(), 1);
        let cmd = &pipeline.commands[0];
        assert!(matches!(&cmd.name, Word::Literal(s) if s == "ls"));
        assert_eq!(cmd.args.len(), 2);
    }

    #[test]
    fn test_pipeline() {
        let script = parse("cat file | grep pattern | head -5").unwrap();
        let pipeline = &script.command_lists[0].first;
        assert_eq!(pipeline.commands.len(), 3);
        assert!(matches!(&pipeline.commands[0].name, Word::Literal(s) if s == "cat"));
        assert!(matches!(&pipeline.commands[1].name, Word::Literal(s) if s == "grep"));
        assert!(matches!(&pipeline.commands[2].name, Word::Literal(s) if s == "head"));
    }

    #[test]
    fn test_and_or() {
        let script = parse("cmd1 && cmd2 || cmd3").unwrap();
        let list = &script.command_lists[0];
        assert_eq!(list.rest.len(), 2);
        assert_eq!(list.rest[0].0, ListOp::And);
        assert_eq!(list.rest[1].0, ListOp::Or);
    }

    #[test]
    fn test_redirect() {
        let script = parse("echo hello > out.txt").unwrap();
        let cmd = &script.command_lists[0].first.commands[0];
        assert_eq!(cmd.redirects.len(), 1);
        assert_eq!(cmd.redirects[0].op, RedirectOp::Write);
    }

    #[test]
    fn test_append_redirect() {
        let script = parse("echo world >> out.txt").unwrap();
        let cmd = &script.command_lists[0].first.commands[0];
        assert_eq!(cmd.redirects.len(), 1);
        assert_eq!(cmd.redirects[0].op, RedirectOp::Append);
    }

    #[test]
    fn test_semicolon_commands() {
        let script = parse("cd /kb; ls").unwrap();
        assert_eq!(script.command_lists.len(), 2);
    }

    #[test]
    fn test_quoted_args() {
        let script = parse("grep 'hello world' file.txt").unwrap();
        let cmd = &script.command_lists[0].first.commands[0];
        assert_eq!(cmd.args.len(), 2);
        assert!(matches!(&cmd.args[0], Word::SingleQuoted(s) if s == "hello world"));
    }

    #[test]
    fn test_glob_pattern() {
        let script = parse("ls *.md").unwrap();
        let cmd = &script.command_lists[0].first.commands[0];
        assert!(matches!(&cmd.args[0], Word::Glob(s) if s == "*.md"));
    }

    #[test]
    fn test_variable_in_command() {
        let script = parse("echo $HOME").unwrap();
        let cmd = &script.command_lists[0].first.commands[0];
        assert!(matches!(&cmd.args[0], Word::Variable(s) if s == "HOME"));
    }

    #[test]
    fn test_empty_input() {
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
    }

    #[test]
    fn test_complex_command() {
        let script =
            parse("find /kb -name '*.md' -type f | xargs grep -l 'deploy' | wc -l").unwrap();
        let pipeline = &script.command_lists[0].first;
        assert_eq!(pipeline.commands.len(), 3);
    }
}
