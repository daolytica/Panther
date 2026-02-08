// Cline agent module - Advanced agentic coding capabilities
// Based on Cline architecture with full tool execution loop

pub mod agent_loop;
pub mod checkpoints;
pub mod error_monitor;
pub mod context_builder;
pub mod tools;

pub use agent_loop::ClineAgentLoop;
// pub use checkpoints::{Checkpoint, create_checkpoint, restore_checkpoint, compare_checkpoint};
// pub use error_monitor::LinterError;
// pub use context_builder::ContextBuilder;
