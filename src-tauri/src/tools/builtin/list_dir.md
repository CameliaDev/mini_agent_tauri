# `list_dir.rs`

- 源码：[`list_dir.rs`](./list_dir.rs)
- 分层：工具层 / 内置工具

## 职责

实现 `list_dir` 工具：列出工作区内一个目录的**直接子项**，不递归。跳过名为 `node_modules`、`target`、`.git` 的目录。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `IGNORED_DIR_NAMES` | `[".git", "node_modules", "target"]` |
| `MAX_LIST_ENTRIES` | 单次最多展示 200 项 |
| `ListDir` | 持有工作区根的工具 |
| `ensure_listable()` | 拒绝忽略目录、拒绝非目录 |
| `ignored_dir_in_path()` | 抵消 `.` / `..` 后，看路径里有没有忽略名 |
| `list_directory()` | 读一层，排序，按上限截断 |
| `DirListing::render()` | 拼给模型看的文本 |
| `impl Tool for ListDir` | `name()` 返回 `"list_dir"` |

## 逻辑要点

1. `path` 可以省略或传空字符串，这时列出工作区根。
2. 展示规则：目录以 `/` 结尾，符号链接以 `@` 结尾，普通文件只有名字。排序按原始名字，不按带后缀的展示名，避免 `/` 改变顺序。
3. **只跳过目录**。一个名叫 `target` 的文件仍然会列出来。
4. 路径任何一段是忽略名都拒绝，包括 `node_modules/pkg`。`node_modules/../src` 抵消之后不再经过 `node_modules`，允许。
5. 符号链接会先被沙箱 `canonicalize`。真实路径如果落进忽略目录，同样拒绝。列条目时用 `file_type()`，不跟随链接，所以不会递归进链接那边。
6. 条目超过上限时，正文写明「共 N 项，只显示前 M 项」。空目录且什么都没跳过时返回 `（空目录）`。

## 单元测试

| 测试 | 锁住什么 |
| --- | --- |
| `lists_one_level_and_skips_junk_dirs` | 跳过三个垃圾目录、不递归、保留普通文件和子目录 |
| `file_named_like_junk_is_listed_but_junk_dir_is_refused` | 同名文件仍列出；拒绝 `node_modules` 及其子目录；`node_modules/../keep` 允许；非目录和 `..` |
| `truncates_long_listings` | 上限生效，被截掉的名字不出现 |
| `shows_symlink_suffix` | 仅 Unix：链接显示为 `name@` |

## 依赖关系

- 依赖：[`tools::Tool`](../mod.md)、[`sandbox`](../sandbox.md)、[`protocol`](../../protocol.md)。
- 被谁使用：[`ToolRegistry::with_workspace()`](../mod.md)。

## 当前行为说明

- 不递归。模型要看子目录，需要再点一次 `list_dir`。
- 工作区根目录自己如果碰巧叫 `target`，列 `"."` 仍然允许，否则根目录会变成无法查看。
- 跳过名单是固定的三个名字，没有配置项。
