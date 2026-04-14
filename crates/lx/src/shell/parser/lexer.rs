//! 词法分析器 (Lexer): 将输入字符串转换为 Token 流
//!
//! 支持的 token 类型:
//! - 单词 (命令名、参数、文件路径)
//! - 操作符 (|, &&, ||, ;, >, >>, <)
//! - 引号 (单引号、双引号)
//! - 变量引用 ($VAR, ${VAR})
//! - Glob 模式 (*, ?)

/// Token 类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// 普通单词 (命令名、参数等)
    Word(String),
    /// 单引号字符串内容
    SingleQuoted(String),
    /// 双引号字符串内容 (可能包含变量引用)
    DoubleQuoted(String),
    /// 变量引用: $VAR 或 ${VAR}
    Variable(String),
    /// 管道操作符: |
    Pipe,
    /// 逻辑 AND: &&
    And,
    /// 逻辑 OR: ||
    Or,
    /// 分号: ;
    Semicolon,
    /// 输出重定向: >
    Redirect,
    /// 追加重定向: >>
    Append,
    /// 输入重定向: <
    RedirectIn,
    /// Stderr 重定向: 2>
    RedirectErr,
    /// 换行
    Newline,
}

/// 词法分析器
pub struct Lexer<'a> {
    input: &'a str,
    chars: Vec<char>,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    /// 将输入转换为 Token 序列
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        while self.pos < self.chars.len() {
            self.skip_whitespace();

            if self.pos >= self.chars.len() {
                break;
            }

            let ch = self.chars[self.pos];

            match ch {
                // 注释: 跳过到行尾
                '#' => {
                    while self.pos < self.chars.len() && self.chars[self.pos] != '\n' {
                        self.pos += 1;
                    }
                }

                // 换行
                '\n' => {
                    tokens.push(Token::Newline);
                    self.pos += 1;
                }

                // 分号
                ';' => {
                    tokens.push(Token::Semicolon);
                    self.pos += 1;
                }

                // 管道 | 或逻辑 OR ||
                '|' => {
                    self.pos += 1;
                    if self.pos < self.chars.len() && self.chars[self.pos] == '|' {
                        tokens.push(Token::Or);
                        self.pos += 1;
                    } else {
                        tokens.push(Token::Pipe);
                    }
                }

                // 逻辑 AND && 或后台 &
                '&' => {
                    self.pos += 1;
                    if self.pos < self.chars.len() && self.chars[self.pos] == '&' {
                        tokens.push(Token::And);
                        self.pos += 1;
                    }
                    // 单个 & 暂不支持后台执行，忽略
                }

                // 输出重定向 > 或追加 >>
                '>' => {
                    self.pos += 1;
                    if self.pos < self.chars.len() && self.chars[self.pos] == '>' {
                        tokens.push(Token::Append);
                        self.pos += 1;
                    } else {
                        tokens.push(Token::Redirect);
                    }
                }

                // 输入重定向 <
                '<' => {
                    tokens.push(Token::RedirectIn);
                    self.pos += 1;
                }

                // 单引号字符串
                '\'' => {
                    let s = self.read_single_quoted()?;
                    tokens.push(Token::SingleQuoted(s));
                }

                // 双引号字符串
                '"' => {
                    let s = self.read_double_quoted()?;
                    tokens.push(Token::DoubleQuoted(s));
                }

                // 变量引用
                '$' => {
                    let var = self.read_variable()?;
                    tokens.push(Token::Variable(var));
                }

                // 数字开头: 可能是 2> stderr 重定向
                '2' if self.peek_at(1) == Some('>') => {
                    self.pos += 2;
                    tokens.push(Token::RedirectErr);
                }

                // 普通单词
                _ => {
                    let word = self.read_word();
                    if !word.is_empty() {
                        // 检查是否包含变量引用 (如 prefix$VAR)
                        tokens.push(Token::Word(word));
                    }
                }
            }
        }

        Ok(tokens)
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len()
            && (self.chars[self.pos] == ' ' || self.chars[self.pos] == '\t')
        {
            self.pos += 1;
        }
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    /// 读取单引号字符串 (不做任何扩展)
    fn read_single_quoted(&mut self) -> Result<String, LexError> {
        self.pos += 1; // skip opening '
        let start = self.pos;

        while self.pos < self.chars.len() && self.chars[self.pos] != '\'' {
            self.pos += 1;
        }

        if self.pos >= self.chars.len() {
            return Err(LexError::UnterminatedString(
                self.input[start..].to_string(),
            ));
        }

        let content: String = self.chars[start..self.pos].iter().collect();
        self.pos += 1; // skip closing '
        Ok(content)
    }

    /// 读取双引号字符串 (保留变量引用标记)
    fn read_double_quoted(&mut self) -> Result<String, LexError> {
        self.pos += 1; // skip opening "
        let mut content = String::new();

        while self.pos < self.chars.len() && self.chars[self.pos] != '"' {
            if self.chars[self.pos] == '\\' && self.pos + 1 < self.chars.len() {
                // 转义字符
                let next = self.chars[self.pos + 1];
                match next {
                    '"' | '\\' | '$' | '`' => {
                        content.push(next);
                        self.pos += 2;
                    }
                    'n' => {
                        content.push('\n');
                        self.pos += 2;
                    }
                    't' => {
                        content.push('\t');
                        self.pos += 2;
                    }
                    _ => {
                        content.push('\\');
                        content.push(next);
                        self.pos += 2;
                    }
                }
            } else {
                content.push(self.chars[self.pos]);
                self.pos += 1;
            }
        }

        if self.pos >= self.chars.len() {
            return Err(LexError::UnterminatedString(content));
        }

        self.pos += 1; // skip closing "
        Ok(content)
    }

    /// 读取变量名: $VAR 或 ${VAR}
    fn read_variable(&mut self) -> Result<String, LexError> {
        self.pos += 1; // skip $

        if self.pos >= self.chars.len() {
            return Ok(String::new());
        }

        // ${VAR} 形式
        if self.chars[self.pos] == '{' {
            self.pos += 1; // skip {
            let start = self.pos;
            while self.pos < self.chars.len() && self.chars[self.pos] != '}' {
                self.pos += 1;
            }
            if self.pos >= self.chars.len() {
                return Err(LexError::UnterminatedVariable(
                    self.input[start..].to_string(),
                ));
            }
            let name: String = self.chars[start..self.pos].iter().collect();
            self.pos += 1; // skip }
            return Ok(name);
        }

        // 特殊变量: $?, $#, $0-$9
        if self.pos < self.chars.len()
            && matches!(self.chars[self.pos], '?' | '#' | '0'..='9' | '@' | '*')
        {
            let ch = self.chars[self.pos];
            self.pos += 1;
            return Ok(ch.to_string());
        }

        // $VAR 形式: 读取连续的字母/数字/下划线
        let start = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_alphanumeric() || self.chars[self.pos] == '_')
        {
            self.pos += 1;
        }

        let name: String = self.chars[start..self.pos].iter().collect();
        Ok(name)
    }

    /// 读取普通单词 (遇到空格、操作符停止)
    fn read_word(&mut self) -> String {
        let start = self.pos;

        while self.pos < self.chars.len() {
            let ch = self.chars[self.pos];
            match ch {
                ' ' | '\t' | '\n' | '|' | '&' | ';' | '>' | '<' | '\'' | '"' | '$' => break, // 变量引用单独处理
                '\\' if self.pos + 1 < self.chars.len() => {
                    self.pos += 2; // 跳过转义
                }
                _ => self.pos += 1,
            }
        }

        self.chars[start..self.pos].iter().collect()
    }
}

/// 词法分析错误
#[derive(Debug, Clone)]
pub enum LexError {
    UnterminatedString(String),
    UnterminatedVariable(String),
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LexError::UnterminatedString(s) => write!(f, "unterminated string: {s}"),
            LexError::UnterminatedVariable(s) => write!(f, "unterminated variable: {s}"),
        }
    }
}

impl std::error::Error for LexError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(input: &str) -> Vec<Token> {
        Lexer::new(input).tokenize().unwrap()
    }

    #[test]
    fn test_simple_command() {
        let tokens = lex("ls -la /kb");
        assert_eq!(
            tokens,
            vec![
                Token::Word("ls".into()),
                Token::Word("-la".into()),
                Token::Word("/kb".into()),
            ]
        );
    }

    #[test]
    fn test_pipe() {
        let tokens = lex("cat file.md | grep hello");
        assert_eq!(
            tokens,
            vec![
                Token::Word("cat".into()),
                Token::Word("file.md".into()),
                Token::Pipe,
                Token::Word("grep".into()),
                Token::Word("hello".into()),
            ]
        );
    }

    #[test]
    fn test_and_or() {
        let tokens = lex("cmd1 && cmd2 || cmd3");
        assert_eq!(
            tokens,
            vec![
                Token::Word("cmd1".into()),
                Token::And,
                Token::Word("cmd2".into()),
                Token::Or,
                Token::Word("cmd3".into()),
            ]
        );
    }

    #[test]
    fn test_redirect() {
        let tokens = lex("echo hello > output.txt");
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".into()),
                Token::Word("hello".into()),
                Token::Redirect,
                Token::Word("output.txt".into()),
            ]
        );
    }

    #[test]
    fn test_append() {
        let tokens = lex("echo world >> output.txt");
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".into()),
                Token::Word("world".into()),
                Token::Append,
                Token::Word("output.txt".into()),
            ]
        );
    }

    #[test]
    fn test_single_quoted() {
        let tokens = lex("grep 'hello world' file.txt");
        assert_eq!(
            tokens,
            vec![
                Token::Word("grep".into()),
                Token::SingleQuoted("hello world".into()),
                Token::Word("file.txt".into()),
            ]
        );
    }

    #[test]
    fn test_double_quoted() {
        let tokens = lex(r#"echo "hello world""#);
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".into()),
                Token::DoubleQuoted("hello world".into()),
            ]
        );
    }

    #[test]
    fn test_variable() {
        let tokens = lex("echo $HOME ${PATH}");
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".into()),
                Token::Variable("HOME".into()),
                Token::Variable("PATH".into()),
            ]
        );
    }

    #[test]
    fn test_semicolon() {
        let tokens = lex("cd /kb; ls");
        assert_eq!(
            tokens,
            vec![
                Token::Word("cd".into()),
                Token::Word("/kb".into()),
                Token::Semicolon,
                Token::Word("ls".into()),
            ]
        );
    }

    #[test]
    fn test_complex_pipeline() {
        let tokens = lex("cat file.md | grep -i 'pattern' | head -5 > out.txt");
        assert_eq!(
            tokens,
            vec![
                Token::Word("cat".into()),
                Token::Word("file.md".into()),
                Token::Pipe,
                Token::Word("grep".into()),
                Token::Word("-i".into()),
                Token::SingleQuoted("pattern".into()),
                Token::Pipe,
                Token::Word("head".into()),
                Token::Word("-5".into()),
                Token::Redirect,
                Token::Word("out.txt".into()),
            ]
        );
    }

    #[test]
    fn test_stderr_redirect() {
        let tokens = lex("cmd 2> error.log");
        assert_eq!(
            tokens,
            vec![
                Token::Word("cmd".into()),
                Token::RedirectErr,
                Token::Word("error.log".into()),
            ]
        );
    }

    #[test]
    fn test_comment() {
        let tokens = lex("ls # this is comment");
        assert_eq!(tokens, vec![Token::Word("ls".into()),]);
    }

    #[test]
    fn test_escaped_chars_in_double_quotes() {
        let tokens = lex(r#"echo "hello\nworld""#);
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".into()),
                Token::DoubleQuoted("hello\nworld".into()),
            ]
        );
    }

    #[test]
    fn test_empty_input() {
        let tokens = lex("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_whitespace_only() {
        let tokens = lex("   \t  ");
        assert!(tokens.is_empty());
    }
}
