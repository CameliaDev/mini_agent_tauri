# `main.rs`

- 源码：[`main.rs`](./main.rs)
- 分层：二进制入口

## 职责

桌面程序的可执行入口。它本身不含业务逻辑，只做两件事：在 Windows release 构建时隐藏额外控制台窗口，然后调用库里的 `run()`。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` | release 版不弹出控制台窗口（模板注释明确要求保留） |
| `fn main()` | 调用 `mini_agent_lib::run()` |

## 依赖关系

- 依赖：`mini_agent_lib`（在 `Cargo.toml` 的 `[lib]` 中指定的库名，实现位于 [`lib.rs`](./lib.rs)）。
- 被谁使用：操作系统启动应用时进入这里。

## 当前行为说明

`bin` 与 `lib` 分离是 Tauri 模板的标准做法：所有可复用代码放在库里，二进制入口只负责转发。
