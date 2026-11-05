# `icons/`

- 目录：`icons/`
- 分层：二进制资源（应用图标）

## 职责

桌面应用图标的各尺寸版本，由 [`tauri.conf.json`](../tauri.conf.md) 的 `bundle.icon` 引用，用于生成安装包、任务栏图标、快捷方式。

## 文件说明

| 文件 | 用途 |
| --- | --- |
| `32x32.png` | 小尺寸图标 / 任务栏 |
| `128x128.png`、`128x128@2x.png` | Linux、macOS 常规与 2 倍图 |
| `icon.png` | 通用源图 |
| `icon.ico` | Windows 安装包与可执行文件图标 |
| `icon.icns` | macOS 应用图标 |
| `Square*Logo.png`、`StoreLogo.png` | Windows / Microsoft Store 磁贴图标（30/44/71/89/107/142/150/284/310 等尺寸） |

## 为什么不为每个 PNG 单独写同名文档

它们是二进制资源，逐一写 `.md` 没有信息增量；整个目录共用这一份说明。这是本项目「同名文档」约定的**例外类别**之一（另一类是自动生成物）。

## 依赖关系

- 被谁使用：[`tauri.conf.json`](../tauri.conf.md) 的 `bundle.icon`。
- 与谁区分：`public/vite.svg` 只影响浏览器标签页，不影响桌面窗口。

## 当前行为说明

- 全部是 `create-tauri-app` 模板自带的 Tauri 默认图标，正式发布前必须换成自有品牌图标。
- 替换时保持文件名不变最省事；否则要同步改 `tauri.conf.json` 里的列表。
- 可用 `npm run tauri icon <源图路径>` 由一张大图自动生成全套尺寸。
