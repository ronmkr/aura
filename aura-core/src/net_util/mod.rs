pub mod logic;
pub mod resolver;
pub use logic::*;
pub use resolver::{create_resolver, ReqwestDnsResolver, TokioResolver};
