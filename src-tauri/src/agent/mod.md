# `mod.rs`（agent 模块）

- 源码：[`mod.rs`](./mod.rs)
- 分层：agent 运行时层的模块入口

## 职责

把「事件协议、会话历史、主循环」三个子模块组织成一个 `agent` 模块，并把主循环以 `agent::run_agent` 的名字导出，让上层不必关心内部文件划分。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `pub mod events` | 流式事件枚举，见 [`events.md`](./events.md) |
| `pub mod runner` | agent 主循环，见 [`runner.md`](./runner.md) |
| `pub mod session` | 消息历史追加函数，见 [`session.md`](./session.md) |
| `pub use runner::run_agent` | 把主循环提升到 `agent` 命名空间 |

## 当前行为说明

主循环文件叫 `runner.rs` 而不是 `loop.rs`：`loop` 是 Rust 关键字，不能直接用作模块名。
