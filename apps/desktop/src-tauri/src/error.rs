//! IPC error shape: `{ code, message }`, message in pt-BR ready for the user (contracts.md §4).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_setting(message: impl Into<String>) -> Self {
        Self::new("invalid_setting", message)
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new("invalid_input", message)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

// Internal details go to the log; the user gets a short pt-BR message (Law 13).
impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        log::error!("database error: {e}");
        Self::new("database", "Erro ao acessar o banco de dados local.")
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        log::error!("io error: {e}");
        Self::new("io", format!("Erro de leitura/gravação de arquivo: {e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        log::error!("json error: {e}");
        Self::new("internal", "Dado interno corrompido.")
    }
}

pub type AppResult<T> = Result<T, AppError>;
