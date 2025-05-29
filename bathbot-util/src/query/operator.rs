pub enum Operator {
    Equal,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl From<&str> for Operator {
    fn from(s: &str) -> Self {
        match s {
            "=" | ":" => Self::Equal,
            "<" => Self::Less,
            "<=" | "<:" => Self::LessOrEqual,
            ">" => Self::Greater,
            ">=" | ">:" => Self::GreaterOrEqual,
            _ => unreachable!(),
        }
    }
}
