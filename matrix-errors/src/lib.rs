use anyhow::Error;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbErr {
    #[error("Database is unreachable: {0:?}")]
    Unreachable(Error),
}
