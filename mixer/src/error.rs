use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::error;

pub type AppResult<T> = core::result::Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    User(UserError),
    Internal(anyhow::Error),
}

#[derive(Debug)]
pub enum UserError {
    InvalidUrl,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::User(UserError::InvalidUrl) => (StatusCode::BAD_REQUEST, "invalid url"),
            AppError::Internal(err) => {
                error!("Internal server error: {err:?}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
            }
        }
        .into_response()
    }
}

impl From<UserError> for AppError {
    fn from(err: UserError) -> Self {
        AppError::User(err)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err)
    }
}
