# `sandbox.rs`

- 源码：[`sandbox.rs`](./sandbox.rs)
- 分层：工具层 / 工作区沙箱

## 职责

给 `read` / `write` / `edit` / `list_dir` 提供同一套边界：路径必须落在工作区根里面，写入用「临时文件 + rename」，读入的必须是不太大的 UTF-8 文本。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `WORKSPACE_ENV` | 环境变量名 `MINI_AGENT_WORKSPACE` |
| `workspace_root_from_env()` | 读环境变量；空或未设置时用当前目录 |
| `load_workspace_root()` | 启动时调用。能规范化就存绝对路径，目录不存在就保留原值 |
| `prepare_root()` | 把根目录 `canonicalize` 成真实绝对路径 |
| `resolve_inside()` | 把用户路径解析到根内；逃逸或指向外部的符号链接返回错误 |
| `read_utf8_file()` | 读一个有大小上限的 UTF-8 文本，拒绝目录和含 NUL 的文件 |
| `atomic_write()` | 同目录写临时文件，`sync_all` 之后 `rename` 覆盖目标 |
| `arg_path` / `arg_string` / `arg_path_or` / `arg_usize_opt` | 从工具参数 JSON 里取值 |
| `TestWorkspace` | 仅测试：临时目录，丢弃时删掉 |

## 逻辑要点

1. **先词法、再真实路径**。`normalize` 不访问磁盘，专门挡住 `../`。目标已经存在时再 `canonicalize`，这一步才能看见符号链接的落点。
2. **用 `Path::starts_with` 而不是字符串前缀**。`/tmp/ws` 包得住 `/tmp/ws/a`，包不住 `/tmp/ws-secret`。
3. **新文件还没有 inode**。这时检查父目录的真实路径是否仍在根内，避免「父目录是指向外部的链接」。
4. **不自动建目录**。`atomic_write` 发现父目录不存在就报错，免得模型少写一级路径时造出一串文件夹。
5. **POSIX 上同目录 `rename` 会原子替换旧文件**。Windows 上目标已存在时 `rename` 会失败，代码先删再改名，中间有极短空窗，说明书里写明了。
6. **参数助手分两种**。路径会 `trim`，空字符串算没传；`contents` / `old_string` / `new_string` 保留原文，空字符串是合法的。

## 单元测试

| 测试 | 锁住什么 |
| --- | --- |
| `relative_and_dot_resolve_inside_root` | 相对路径、`.`、根内绝对路径 |
| `dot_dot_staying_inside_is_allowed` | `sub/../a.txt` 抵消之后仍在根内 |
| `escapes_are_rejected` | `../`、绝对路径、空路径、NUL |
| `sibling_prefix_is_not_inside` | `ws` 与 `ws-secret` 不能靠字符串前缀混进去 |
| `symlink_escape_is_rejected` | 仅 Unix：链接本身或父目录指向外部都拒绝，且不会把文件写出去 |
| `missing_workspace_root_errors` | 根目录不存在时错误里带「不存在」 |
| `atomic_write_replaces_without_leaving_temp` | 覆盖写入、不留 `.mini-agent-tmp-*`、父目录缺失时不创建 |

## 依赖关系

- 依赖：标准库路径、文件 IO、`anyhow`。
- 被谁使用：四个文件工具、[`bash`](./builtin/bash.md)（用 `prepare_root` 和参数助手）、[`ToolRegistry::builtin()`](./mod.md)、[`AppState::from_env()`](../app/state.md)。

## 当前行为说明

- 沙箱挡的是「逃出根目录」，不是「隐藏根目录里的秘密」。工作区里的 `.env` 可以被 `read` 读到。
- 判定用的是规范化后的真实路径。工作区的另一条符号链接别名，词法上不在根下面，会被拒绝。
- `exists` 和 `canonicalize` 之间有时间差。这是教学实现，没有做更重的开目录句柄校验。
- 目录项的 `fsync` 没做。掉电时个别文件系统上，`rename` 本身仍可能丢失；文件内容在改名前已经 `sync_all`。
