# `Cargo.lock`

- 源码：[`Cargo.lock`](./Cargo.lock)
- 分层：后端依赖锁文件（**自动生成，勿手改**）

## 职责

锁定 [`Cargo.toml`](./Cargo.md) 解析出的完整依赖树（含间接依赖）的确切版本与校验和。桌面应用要能复现构建，所以应用 crate 的锁文件应当入库（库 crate 通常不入库）。

## 规则

| 该做 | 不该做 |
| --- | --- |
| 改 `Cargo.toml` 后运行 `cargo check`，让 Cargo 自动更新并一起提交 | 手改版本号或 `checksum` |
| 离线构建时用 `cargo --offline`，依赖必须已在本地缓存 | 假设 CI 一定能联网拉新版本 |

## 依赖关系

- 由谁生成：[`Cargo.toml`](./Cargo.md)。
- 被谁使用：`cargo check` / `cargo test` / `cargo clippy` / `cargo build`。

## 当前行为说明

- `rusqlite` 带 `bundled` feature，会连带编译 `libsqlite3-sys`（SQLite C 源码），首次构建较慢；产物缓存在 `target/`（已忽略，不入库）。
- 离线环境下 `cargo` 需要本地 registry 已有对应 crate 源码，否则会报 `failed to download`。
