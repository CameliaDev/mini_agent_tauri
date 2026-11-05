# `index.html`

- 源码：[`index.html`](./index.html)
- 分层：前端入口 HTML

## 职责

Vite 的 HTML 模板：提供一个 `#root` 挂载点，并加载 `src/main.tsx` 这个模块入口。

## 关键内容

| 部分 | 作用 |
| --- | --- |
| `<meta charset="UTF-8">` | 声明 UTF-8，中文才不会乱码 |
| `<meta name="viewport" ...>` | 桌面窗口缩放时让 CSS 像素与视口对应 |
| `<link rel="icon" ... href="/vite.svg">` | 浏览器标签图标；Tauri 窗口图标由 `tauri.conf.json` 的 `bundle.icon` 决定，两者是两处 |
| `<div id="root">` | React 挂载点，`main.tsx` 里 `getElementById("root")` 找的就是它 |
| `<script type="module" src="/src/main.tsx">` | 模块入口；`type="module"` 让 Vite 能做 ESM 与热更新 |

## 依赖关系

- 被谁使用：Vite 构建时作为模板；[`src/main.tsx`](./src/main.md) 是它加载的脚本。
- 相关配置：[`vite.config.md`](./vite.config.md)、[`src-tauri/tauri.conf.md`](./src-tauri/tauri.conf.md)。

## 当前行为说明

- 页面标题写死为 `mini_agent`，与 `tauri.conf.json` 的 `productName` 是两处，改名字要一起改。
- `<html lang="en">` 与实际中文界面不符，属于模板残留；要修的话改成 `zh-CN` 即可（不影响功能）。
