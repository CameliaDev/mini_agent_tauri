# `gen/schemas/`

- 目录：`gen/schemas/`
- 分层：**自动生成物**（Tauri CLI 产出，勿手改）

## 职责

这四个 JSON 文件是 Tauri CLI 根据当前插件与权限生成的 JSON Schema，供编辑器对配置文件做补全和校验：

| 文件 | 服务于 |
| --- | --- |
| `acl-manifests.json` | ACL 清单，描述各插件暴露了哪些权限 |
| `capabilities.json` | Capability 文件的结构校验 |
| `desktop-schema.json` | [`tauri.conf.json`](../../tauri.conf.md) 与 [`capabilities/default.json`](../../capabilities/default.md) 的 `$schema` |
| `windows-schema.json` | Windows 平台相关配置的结构校验 |

## 规则

| 该做 | 不该做 |
| --- | --- |
| 由 `tauri dev` / `tauri build` 自动重新生成 | 手工编辑里面任何一个字段 |
| 让 `tauri dev` 重新生成本目录（各 JSON **不入库**，由 Tauri CLI 产出） | 当作项目源码去 Review 或手工编辑 |

## 为什么不为每个 schema 单独写同名文档

它们是机器生成的、内容随依赖版本变化，手写说明会立刻过时；因此整个目录共用这一份说明。这是本项目「同名文档」约定的**唯一例外类别**：自动生成物用目录级 `README.md` 覆盖。

## 入库情况

目录里的四个 JSON 由 `src-tauri/.gitignore` 的 `/gen/schemas/*` 排除，**不进版本库**（Tauri 模板的默认设计：clone 后运行 `tauri dev` 会自动生成）。唯一例外是这份 `README.md`，用 `!/gen/schemas/README.md` 放行，保证仓库里的文档链接有效。

## 依赖关系

- 由谁生成：Tauri CLI（读取 `Cargo.toml` 的插件与 `capabilities/`）。
- 被谁使用：编辑器 JSON Schema 补全。
