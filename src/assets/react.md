# `react.svg`

- 源码：[`react.svg`](./react.svg)
- 分层：静态资源（模板图标）

## 职责

React 的 Logo，模板自带的示例资源。

## 依赖关系

- 当前**没有被任何组件引用**；界面里的助手头像由 CSS 绘制（[`App.css.md`](../App.css.md) 的 `.msg__avatar`），不用图片。

## 当前行为说明

- 属于可删除的模板残留。删除前确认没有 `import ... from "./assets/react.svg"`；本项目的[`tsconfig.md`](../../tsconfig.md) 打开了 `noUnusedLocals`，多余导入会直接编译失败，所以检查起来很容易。
