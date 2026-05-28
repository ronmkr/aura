use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

/// Trait for protocol-specific task state extensions.
pub trait TaskExtension: Debug + Send + Sync {
    /// Allows downcasting to a concrete type.
    fn as_any(&self) -> &dyn Any;

    /// Allows downcasting an Arc to a concrete type.
    fn as_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
}
