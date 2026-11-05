# `extensions.json`

- 源码：[`extensions.json`](./extensions.json)
- 分层：编辑器推荐插件（IDE 配置）

## 职责

向打开本仓库的开发者推荐两个插件：

| 插件 | 作用 |
| --- | --- |
| `tauri-apps.tauri-vscode` | `tauri.conf.json` 补全、运行 `tauri dev` 的快捷命令 |
| `rust-lang.rust-analyzer` | Rust 代码补全、跳转、类型提示 |

## 为什么它是唯一被放行的 `.vscode` 文件

[`.gitignore`](../.gitignore.md) 里写了 `.vscode/*` 再 `!.vscode/extensions.json`：本机个性化配置（尤其含绝对路径的 `settings.json`）不该入库，但插件推荐对复刻者有价值，所以白名单放行。

## 依赖关系

- 相关：[`.gitignore.md`](../.gitignore.md)。

## 当前行为说明

- 只是推荐，不强制安装；不装也能用 `npm run tauri dev` 跑起来。
