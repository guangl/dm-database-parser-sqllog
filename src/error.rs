use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid format")]
    InvalidFormat,

    #[error("file not found or inaccessible: {0}")]
    FileNotFound(String),

    #[error("empty input: no lines provided")]
    EmptyInput,

    #[error("invalid record start line: line does not match expected format")]
    InvalidRecordStartLine,

    #[error("line too short: expected at least 25 characters, got {0}")]
    LineTooShort(usize),

    #[error("missing closing parenthesis in meta section")]
    MissingClosingParen,

    #[error("insufficient meta fields: expected at least 7 fields, got {0}")]
    InsufficientMetaFields(usize),

    #[error("invalid EP format: expected 'EP[number]', got '{0}'")]
    InvalidEpFormat(String),

    #[error("failed to parse EP number: {0}")]
    EpParseError(String),

    #[error("invalid field format: expected '{expected}', got '{actual}'")]
    InvalidFieldFormat { expected: String, actual: String },

    #[error("failed to parse {field} as integer: {value}")]
    IntParseError { field: String, value: String },

    #[error("failed to parse indicators: {0}")]
    IndicatorsParseError(String),
}
