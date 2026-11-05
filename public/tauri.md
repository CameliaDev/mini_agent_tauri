# `tauri.svg`

- 源码：[`tauri.svg`](./tauri.svg)
- 分层：静态资源（模板图标）

## 职责

Tauri 的 Logo，模板自带的示例资源。

## 依赖关系

- 当前**没有被任何代码引用**（[`index.html`](../index.md) 用的是 `vite.svg`）。

## 当前行为说明

- 属于可删除的模板残留；删除不会影响构建。若以后要在空态或关于页展示 Tauri 品牌，可用 `<img src="/tauri.svg" />`。
