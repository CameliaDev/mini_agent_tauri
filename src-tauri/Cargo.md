# `Cargo.toml`

- 源码：[`Cargo.toml`](./Cargo.toml)
- 分层：后端依赖与构建清单

## 职责

声明 Rust crate 的元信息、依赖、构建脚本与发布优化选项。`cargo check` / `cargo test` / `cargo clippy` 都读它。

## crate 结构

| 配置 | 值 | 为什么 |
| --- | --- | --- |
| `[lib] name` | `mini_agent_lib` | Windows 上 lib 与 bin 同名会冲突，加 `_lib` 后缀避开（详见注释里的 Cargo issue 8519） |
| `crate-type` | `staticlib` / `cdylib` / `rlib` | Tauri 需要动态库给前端调用，同时保留 rlib 供 `cargo test` 链接 |
| `edition` | `2024` | Rust 2024 版次 |
| `[build-dependencies]` | `tauri-build = "2"` | 构建脚本 [`build.rs`](./build.md) 调用它生成配置 |

## 依赖分组（按职责）

| 组 | 依赖 | 用途 |
| --- | --- | --- |
| Tauri | `tauri`、`tauri-plugin-opener` | 窗口、命令、IPC、打开外部链接 |
| 序列化 | `serde`、`serde_json` | 前后端协议结构、工具参数 |
| HTTP / 流 | `reqwest`（`json`, `stream`, `multipart`）、`futures-util` | 调 DeepSeek 接口、读 SSE 字节流 |
| 异步 | `tokio`（`full`）、`tokio-stream`、`async-stream`、`async-trait` | 运行时、通道、trait 上的 async 方法 |
| 错误处理 | `anyhow`、`thiserror` | 命令层统一转成字符串错误 |
| 配置 | `dotenvy` | 启动时读 `.env` |
| 日志 | `log`、`env_logger`、`tracing`、`tracing-subscriber` | 预留的日志能力 |
| 工具 | `chrono`、`uuid`、`regex`、`rand` | 内置工具与通用能力 |
| 序列化扩展 | `serde_yaml` | 预留 |
| 本地存储 | `rusqlite`（`bundled`） | **SQLite 会话持久化**；`bundled` 把 SQLite 一起编进来，不依赖系统库 |

## `[profile.release]`

| 配置 | 作用 |
| --- | --- |
| `codegen-units = 1` | 单个代码生成单元，优化更充分 |
| `lto = true` | 链接期优化 |
| `opt-level = 3` | 最高优化等级 |
| `panic = "abort"` | 遇 panic 直接终止，去掉展开代码 |
| `strip = true` | 去掉符号表，安装包更小 |

代价是**发布构建明显更慢**，只用于 `npm run tauri build`；开发时用 `cargo check` 不受影响。

## 依赖关系

- 与谁配对：[`Cargo.lock`](./Cargo.lock)（锁定版本）、[`tauri.conf.md`](./tauri.conf.md)、[`build.md`](./build.md)。

## 当前行为说明

- 依赖里有若干当前未直接使用的 crate（`thiserror`、`log`、`env_logger`、`tracing*`、`tokio-stream`、`async-stream`、`chrono`、`uuid`、`regex`、`rand`、`serde_yaml`），属于模板与预留能力；精简时移除即可，但会触发一次全量重编译。
- `version: 0.1.0` 与 `tauri.conf.json`、`package.json` 是三处，发版要一起改。
