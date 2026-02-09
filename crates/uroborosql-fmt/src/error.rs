use thiserror::Error;

#[derive(Error, Debug)]
pub enum UroboroSQLFmtError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Illegal operation error: {0}")]
    IllegalOperation(String),
    #[error("Unexpected syntax error: {0}")]
    UnexpectedSyntax(String),
    #[error("Unimplemented Error: {0}")]
    Unimplemented(String),
    #[error("File not found error: {0}")]
    FileNotFound(String),
    #[error("Illegal setting file error: {0}")]
    IllegalSettingFile(String),
    #[error("Rendering Error: {0}")]
    Rendering(String),
    #[error("Runtime Error: {0}")]
    Runtime(String),
    #[error("Validation Error: {error_msg}")]
    Validation {
        // テストでしか使用しておらず、clippy で警告が出るため _ を付与
        _format_result: String,
        error_msg: String,
    },
}
