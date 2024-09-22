use crate::error::AppError;

pub type ModelResult<T> = Result<T, ModelError>;

#[derive(Debug)]
pub enum ModelError {
    SqlError(String),
}

impl From<ModelError> for AppError {
    fn from(err: ModelError) -> AppError {
        match err {
            ModelError::SqlError(s) => AppError::InternalServerError(s),
        }
    }
}
