#[derive(Debug)]
pub enum OppaiErr {
    Binding(String),
    More(String),
    Syntax(String),
    Truncated(String),
    NotImplemented(String),
    IO(String),
    Format(String),
    OOM(String),
    UnexpectedCode(String),
    MissingPath(String),
}

impl OppaiErr {
    pub(crate) fn new(code: i32, msg: impl AsRef<str>) -> Self {
        let msg = String::from(msg.as_ref());
        match code {
            -1 => OppaiErr::More(msg),
            -2 => OppaiErr::Syntax(msg),
            -3 => OppaiErr::Truncated(msg),
            -4 => OppaiErr::NotImplemented(msg),
            -5 => OppaiErr::IO(msg),
            -6 => OppaiErr::Format(msg),
            -7 => OppaiErr::OOM(msg),
            _ => OppaiErr::UnexpectedCode(format!(
                "Expected error codes -1 to -7, got {}: {}",
                code, msg
            )),
        }
    }
}
