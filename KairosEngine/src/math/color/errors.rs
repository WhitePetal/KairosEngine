use std::fmt;


#[derive(Debug, Clone)]
pub enum Color32ParseError {
    // 十六进制字符串长度错误（要求 6 或 8）
    InvalidLength(usize),
    // 十六进制字符解析失败（比如包含非 0-9/A-F 字符）
    InvalidHexChar(String),
    // 空字符串
    EmptyString,
    // 没有 # 头
    NoHead,
}

impl fmt::Display for Color32ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color32ParseError::InvalidLength(len) => {
                write!(f, "Invalide hexadecimal length: {}, only support 6(#RRGGBB) or 8(#RRGGBBAA) bit", len)
            }
            Color32ParseError::InvalidHexChar(s) => {
                write!(f, "Invalide hexadecimal char: {}, only support 0-9, A-F, a-f", s)
            }
            Color32ParseError::EmptyString => {
                write!(f, "The hexadecimal string is null")
            }
            Color32ParseError::NoHead => {
                write!(f, "Invalide hexadecimal string: not start with #")
            }
        }
    }
}

impl std::error::Error for Color32ParseError {
    
}