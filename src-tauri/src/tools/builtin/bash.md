# `bash.rs`

- 源码：[`bash.rs`](./bash.rs)
- 分层：工具层 / 内置工具

## 职责

实现 `bash` 工具：在工作区根目录里执行一条命令，供模型跑测试、构建或 `git status`。

它同时做四件限流：超时、stdout/stderr 截断、危险命令拒绝，以及用户点停止时杀掉子进程。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `Bash` | 持有工作区根，以及可选的 `CancelSignal` |
| `impl Tool for Bash` | `name()` 返回 `"bash"` |
| `timeout_from_args()` | 缺省 30 秒；`0` 和大于 120 都是错误 |
| `run_command()` | 启动进程，在结束、超时、取消三者里等先发生的那个 |
| `command_rejected()` | 启动前的粗黑名单。返回拒绝原因，或 `None` |
| `read_capped()` | 每个流只保留前 16 KiB，其余继续读完，避免管道堵死 |
| `kill_process_tree()` | Unix 杀进程组；Windows 用 `taskkill /T` |

## 逻辑要点

1. 工具名是 `bash`，给模型看。Unix 实际执行 `sh -c`，因为最小环境里不一定有 bash；Windows 执行 `cmd /C`。两边语法不同。
2. 当前目录固定是 `prepare_root` 之后的工作区根。没有 `cwd` 参数，模型不能把命令送到工作区外面去启动。
3. 标准输入是空的，避免命令停在那里等键盘。`DEEPSEEK_API_KEY` 和 `MINI_AGENT_API_KEY` 会从子进程环境里去掉，其它环境变量（包括 `PATH`）照常继承。
4. 非 0 退出码仍然返回 `Ok`，正文以 `exit: N` 开头，后面是 stdout 和 stderr。模型要能看见测试失败。超时、取消、启动失败、护栏拒绝才是 `Err`。
5. `timeout_secs` 不写就是 30。写了 0，或大于 120，直接报错，不把数字偷偷改小。
6. 每个流最多保留 16 KiB。截断之后读任务还要继续把管道读空，否则子进程写满管道就再也不会退出。
7. 护栏在 `spawn` 之前。它按空白和 `;` `&` `|` 分词，不理解引号，也不是系统调用沙箱。`rm` 必须同时带递归和强制，并且目标是 `/`、`~`、`$HOME`、`.`、`..`、`*` 这一类，才会拒绝。`rm -rf target` 这种清构建目录的写法放行。
8. Unix 上子进程放进新的进程组（`process_group(0)`）。取消或超时时对进程组发 `SIGKILL`，shell 拉起的 `sleep` 会一起死。Windows 没有这个调用，改用 `taskkill /F /T /PID` 杀进程树。
9. `run_agent` 把同一份 `CancelSignal` 交进来。`select!` 里取消优先于进程退出，进程退出优先于超时。信号是 `None` 时（`with_workspace` 那条测试路径）命令照常跑，但没人能中途杀掉它。

## 单元测试

| 测试 | 锁住什么 |
| --- | --- |
| `guard_allows_ordinary_commands_and_blocks_dangerous_ones` | 放行 `echo` / `rm notes.txt` / `rm -rf target`；拒绝 `rm -rf /`、`mkfs`、`dd of=/dev/`、fork bomb、关机、`format c:`。不启动进程 |
| `rejects_bad_arguments_without_spawning` | 空命令、缺参数、超时 0 / 121 / 负数 |
| `rejects_dangerous_command_without_spawning` | `rm -rf / && touch pwned` 被拒绝，工作区里没有 `pwned` |
| `runs_in_workspace_and_reports_exit_code` | 仅 Unix：`pwd` 是工作区根，退出码 3 仍是 `Ok`，stdout 和 stderr 都在 |
| `timeout_kills_the_child` | 仅 Unix：`sleep 5` 配 1 秒超时，错误里有「超时」，并且远早于 5 秒返回 |
| `cancel_kills_before_the_side_effect` | 仅 Unix：取消后 `sleep && touch` 不会留下文件 |
| `already_cancelled_does_not_start` | 仅 Unix：信号已经是取消时，错误里有「未启动」 |
| `truncates_output_without_deadlocking` | 仅 Unix：只保留前几个字节；256 KiB 的输出也能在超时前结束 |

进程相关的测试只在 Unix 上跑。护栏和参数测试在任何平台上都会跑。

## 依赖关系

- 依赖：[`tools::Tool`](../mod.md)、[`sandbox`](../sandbox.md) 的 `prepare_root` / `arg_string` / `arg_usize_opt`、[`CancelSignal`](../../agent/cancel.md)、`tokio::process`。
- 被谁使用：[`ToolRegistry::with_workspace_and_cancel()`](../mod.md) 注册后，由 [`run_agent`](../../agent/runner.md) 在模型点名时执行。

## 当前行为说明

- **Unix**：`sh -c`，新进程组，`SIGKILL` 杀整组。`kill_on_drop` 只是兜底，它只杀 shell 自己。shell 已经正常退出之后留在后台的进程不会被再杀；取消和超时发生在 shell 还活着的时候，进程组信号能打到它们。
- **Windows**：`cmd /C`，没有进程组。停止时调用 `taskkill /F /T /PID`，再 `start_kill` 补一刀。这条路径没有在 CI 里跑过。
- 读任务如果 2 秒内没有结束，就丢掉还没拿到的输出，避免工具调用一直卡着。
- 护栏会误伤。`echo "rm -rf /"`、`git commit -m "rm -rf /"` 都会被拒绝，因为引号被直接丢掉。`dd ... of=/dev/null` 也会被拒绝。
- 护栏会漏。`cd /` 之后再删除，不在黑名单里的设备名，以及没有被分词规则切开的写法，都可能放行。它不是容器，也不是 seccomp。
- 被信号杀死的进程，退出码显示成 `exit: -1`（`ExitStatus::code()` 在这种情况下是空的）。超时和取消不走这个字段，它们是 `Err`。
- 工作区里的其它环境变量对子进程可见。只去掉了两个 API Key 的变量名。
