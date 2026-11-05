# `edit.rs`

- 源码：[`edit.rs`](./edit.rs)
- 分层：工具层 / 内置工具

## 职责

实现 `edit` 工具：在工作区文本文件里，把**恰好出现一次**的 `old_string` 换成 `new_string`。零次或多次都不改文件。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `EditFile` | 持有工作区根的工具 |
| `EditReject` | `EmptyOld` / `Identical` / `NotFound` / `Multiple(n)` |
| `apply_unique_edit()` | 纯函数：唯一匹配才替换 |
| `count_non_overlapping()` | 与 `str::replacen` 一致的不重叠计数 |
| `impl Tool for EditFile` | `name()` 返回 `"edit"` |

整文件上限 2 MiB。再大就拒绝，避免为了数匹配次数把巨型文件读进内存。

## 逻辑要点

1. `old_string` 为空会在每个位置都「匹配上」，所以直接报错，不去搜。
2. `old_string` 与 `new_string` 相同也报错：没有改动就不写盘。
3. 计数是**不重叠**的。`"aaa"` 里的 `"aa"` 只算 1 处，替换后剩下最后一个 `a`。这和真正执行的 `replacen` 一致。
4. 先读全文，匹配失败就在 `atomic_write` 之前返回，磁盘内容不变。
5. `new_string` 允许为空，表示删掉那一段唯一匹配。空格和换行都算在匹配里，不会被 `trim`。

## 单元测试

| 测试 | 锁住什么 |
| --- | --- |
| `non_overlapping_count_and_reject_reasons` | 不重叠计数、空旧串、相同新旧串、找不到、多处 |
| `replaces_one_match_including_newlines` | 含换行和中文的唯一替换会写回 |
| `zero_or_many_matches_do_not_change_file` | 两处、零处、空 `old_string` 都不改文件 |
| `rejects_escape` | 逃出工作区时外部文件保持原样 |

## 依赖关系

- 依赖：[`tools::Tool`](../mod.md)、[`sandbox`](../sandbox.md)、[`protocol`](../../protocol.md)。
- 被谁使用：[`ToolRegistry::with_workspace()`](../mod.md)。

## 当前行为说明

- 匹配是字节级的精确比较，没有模糊空白、也没有正则。
- 多处匹配的错误会带上次数，提示模型把 `old_string` 写得更长以便唯一定位。
