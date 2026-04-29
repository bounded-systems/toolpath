#![doc = include_str!("../README.md")]

pub mod error;
pub mod io;
pub mod paths;
pub mod reader;
pub mod types;

pub use error::{ConvoError, Result};
pub use io::ConvoIO;
pub use paths::PathResolver;
pub use reader::DbReader;
pub use types::{
    AssistantMessage, Message, MessageData, Part, PartData, Project, Session, SessionMetadata,
    ToolState, UserMessage,
};

pub mod provider;
pub use provider::{OpencodeConvo, native_name, to_view, tool_category};

pub mod derive;
pub mod project;
