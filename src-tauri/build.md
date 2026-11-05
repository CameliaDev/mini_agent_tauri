# `build.rs`

- 源码：[`build.rs`](./build.rs)
- 分层：Cargo 构建脚本（不属于运行时代码）

## 职责

Tauri 官方模板的构建脚本。Cargo 在编译本 crate 之前执行它，调用 `tauri_build::build()` 处理 Tauri 相关的构建期工作（读取 `tauri.conf.json`、生成上下文与资源等）。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `fn main()` | 构建脚本入口，只有一句 `tauri_build::build()` |

## 依赖关系

- 依赖：`Cargo.toml` 中 `[build-dependencies]` 的 `tauri-build = "2"`。
- 触发者：Cargo 构建流程（`cargo check` / `cargo build` / `tauri dev` 都会先跑它）。

## 当前行为说明

模板自带内容，运行时逻辑不在这里；一般不需要修改。
