# `capabilities/default.json`

- 源码：[`default.json`](./default.json)
- 分层：Tauri 权限清单（Capability）

## 职责

声明「哪些窗口能用哪些插件能力」。Tauri 2 用**默认拒绝**的权限模型：没在这里列出的能力，前端 `invoke` 也调不动。

## 字段

| 字段 | 值 | 作用 |
| --- | --- | --- |
| `$schema` | `../gen/schemas/desktop-schema.json` | 给编辑器提供补全和校验；schema 由 Tauri CLI 自动生成 |
| `identifier` | `default` | 这份能力清单的名字 |
| `windows` | `["main"]` | 只作用于主窗口（`tauri.conf.json` 里窗口未显式指定 label 时默认 `main`） |
| `permissions` | `core:default`、`opener:default` | 核心能力（含 `invoke` / `Channel`）与打开外部链接能力 |

## 为什么不需要为自定义命令加权限

我们自己写的 `chat`、`list_sessions` 等 `#[tauri::command]` 属于应用自身命令，`core:default` 已允许前端调用；Capability 主要管的是**插件**和敏感系统 API。

## 依赖关系

- 依赖：[`tauri.conf.md`](../tauri.conf.md)（窗口 label）、[`app/commands.md`](../src/app/commands.md)（被允许调用的命令）。
- `gen/schemas/*.json` 是生成物，不要手改。

## 当前行为说明

- 权限集保持最小：只给了 `core:default` 和 `opener:default`，没有文件系统、shell 等敏感权限。
- 若以后加了需要插件的命令（比如 `tauri-plugin-fs`），必须在这里补对应 permission，否则前端调用会被拒绝。
