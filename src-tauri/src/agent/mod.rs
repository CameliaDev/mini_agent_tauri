// 声明 cancel 子模块，Rust 会去加载 src/agent/cancel.rs
pub mod cancel;
// 声明 events 子模块，Rust 会去加载 src/agent/events.rs
pub mod events;
// 声明 runner 子模块，Rust 会去加载 src/agent/runner.rs
// （runner 而不是 loop，因为 loop 是 Rust 关键字，不能做模块名）
pub mod runner;
// 声明 session 子模块，Rust 会去加载 src/agent/session.rs
pub mod session;

// 把主循环导出成 agent::run_agent，方便 command 层调用
pub use runner::run_agent;
