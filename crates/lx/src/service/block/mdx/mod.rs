pub mod emitter;
pub mod parser;

// Re-export core functions for convenience
pub use emitter::emit_mdx;
pub use parser::parse_mdx;
