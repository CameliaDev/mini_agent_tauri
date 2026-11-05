# `vite.svg`

- 源码：[`vite.svg`](./vite.svg)
- 分层：静态资源（模板图标）

## 职责

Vite 的 Logo。被 [`index.html`](../index.md) 以 `<link rel="icon" href="/vite.svg">` 引用，只影响**浏览器**标签页图标。

## 关键点

放在 `public/` 里的文件会被 Vite 原样复制到 `dist/` 根目录，路径以 `/` 开头直接访问，不经过打包和哈希。

## 依赖关系

- 被谁使用：[`index.html`](../index.md)。
- 与谁区分：[`src-tauri/tauri.conf.md`](../src-tauri/tauri.conf.md) 的 `bundle.icon` 决定**桌面窗口/安装包**图标，两者互不影响。

## 当前行为说明

- 这是 `create-tauri-app` 模板残留；正式产品应换成自己的品牌图标，并同步 `index.html` 与 `tauri.conf.json`。
