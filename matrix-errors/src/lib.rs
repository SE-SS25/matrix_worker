use anyhow::Error;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbErr {
    #[error("Database is unreachable: {0:?}")]
    Unreachable(Error),
}

#[derive(Debug, Error)]
pub enum MongoErr {
    #[error("Mongo is unreachable: {0:?}")]
    Unreachable(Error),
    #[error("Invalid Mongo URL for id: {0:?}")]
    InvalidUrl(String),
}

#[derive(Debug, Error)]
pub enum MatrixErr {
    #[error("Room {0:?} already exists, can't create")]
    RoomAlreadyExists(String),
    #[error("Room name {0:?} is not allowed")]
    IllegalRoomName(String),
    #[error("The room {0:?} does not exist")]
    RoomNotFound(String),
    #[error("You are not a member of room {0:?}")]
    NotInRoom(String),
    #[error("General error: {0}")]
    General(String),
}
