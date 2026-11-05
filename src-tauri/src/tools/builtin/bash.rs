// ============================================================================
// bash 工具：在工作区根目录里跑一条命令
//
// 模型用它跑测试、构建、git status。名字叫 bash，是给模型看的；
// Unix 上实际执行的是 sh -c（不要求机器装了 bash），Windows 上是 cmd /C。
//
// 这一层做四件事：
//   1. 超时：到点就杀进程，默认 30 秒，最长 120 秒，0 和更大的数直接拒绝
//   2. 截断：stdout / stderr 各只保留前 16 KiB，多出来的继续读掉，避免管道堵死
//   3. 护栏：启动前用空白分词挡掉 rm -rf /、格式化磁盘、关机。这不是 shell 解析器
//   4. 取消：用户点停止时，同一份 CancelSignal 会把子进程杀掉
// ============================================================================

// 引入 async_trait 宏，让 impl 块里可以写 async fn
use async_trait::async_trait;
// 引入标准输入输出的配置
use std::process::Stdio;
// 引入路径类型
use std::path::{Path, PathBuf};
// 引入超时时长
use std::time::Duration;
// 引入异步读取，用来把子进程的输出读完
use tokio::io::{AsyncRead, AsyncReadExt};
// 引入取消信号。用户点停止时，这里要能结束正在跑的命令
use crate::agent::cancel::CancelSignal;
// 引入工具说明书相关的数据结构
use crate::protocol::{ToolDefinition, ToolFunction};
// 引入参数解析和工作区根的检查
use crate::tools::sandbox::{arg_string, arg_usize_opt, prepare_root};
// 引入工具统一接口
use crate::tools::Tool;

// 不写 timeout_secs 时用这个。日常的测试和 git status 够用，又不会无限挂着
const DEFAULT_TIMEOUT_SECS: u64 = 30;
// 再长就拒绝，不偷偷改成 120。模型该把任务拆开
const MAX_TIMEOUT_SECS: u64 = 120;
// 每个流保留的字节数。16 KiB 足够看测试失败，又不会把整份日志塞进上下文
const MAX_OUTPUT_BYTES: usize = 16 * 1024;

// Unix 上用 libc 的 kill。负的 pid 表示「整个进程组」，9 是 SIGKILL。
// 不引入 libc crate：这里只用这一个函数。
#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

// 定义内置工具：在工作区里执行一条命令
pub struct Bash {
    // 工作区根。命令的当前目录固定是它，模型不能另传 cwd
    root: PathBuf,
    // 这一轮对话的取消信号。None 表示没人能取消（注册表的测试路径）
    cancel: Option<CancelSignal>,
}

impl Bash {
    // root 是工作区，cancel 是 run_agent 订到的那一份信号
    pub fn new(root: impl Into<PathBuf>, cancel: Option<CancelSignal>) -> Self {
        Self {
            root: root.into(),
            cancel,
        }
    }
}

// 为它实现 Tool 接口
#[async_trait]
impl Tool for Bash {
    // 工具名。模型和注册表都用这个字符串
    fn name(&self) -> &'static str {
        "bash"
    }

    // 工具说明书
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            kind: "function".to_string(),
            function: ToolFunction {
                name: self.name().to_string(),
                description: format!(
                    "在工作区根目录执行一条命令。Unix 用 sh -c，Windows 用 cmd /C。用于跑测试、构建或 git status。标准输入是空的。stdout 和 stderr 各最多保留 {MAX_OUTPUT_BYTES} 字节。timeout_secs 默认 {DEFAULT_TIMEOUT_SECS}，最大 {MAX_TIMEOUT_SECS}。删根目录、格式化磁盘、关机会被拒绝。用户点停止时进程会被杀掉。"
                ),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "要执行的命令。当前目录固定为工作区根，不能改"
                        },
                        "timeout_secs": {
                            "type": "integer",
                            "description": format!("超时秒数，默认 {DEFAULT_TIMEOUT_SECS}，最大 {MAX_TIMEOUT_SECS}。到点进程会被杀掉")
                        }
                    },
                    "required": ["command"]
                }),
            },
        }
    }

    // 执行：先检查参数和护栏，通过了才启动进程
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<String> {
        // 取出命令。空字符串和纯空白都拒绝，避免 sh -c "" 假装成功
        let raw = arg_string(&args, "command")?;
        let command = raw.trim();
        if command.is_empty() {
            anyhow::bail!("命令不能为空");
        }
        // 超时不合法就不要启动。0 和超过上限都是调用错误，不偷偷改数值
        let timeout = timeout_from_args(&args)?;
        // 护栏在 spawn 之前。被拒绝的命令不会碰到文件系统
        if let Some(reason) = command_rejected(command) {
            anyhow::bail!("{reason}");
        }
        // 根目录必须真实存在。相对路径按此刻的当前目录解析
        let root = prepare_root(&self.root)?;
        // execute 拿到的是 &self，等待取消需要一份可变的订阅
        run_command(
            &root,
            command,
            timeout,
            MAX_OUTPUT_BYTES,
            self.cancel.clone(),
        )
        .await
    }
}

// 解析 timeout_secs。缺省用默认值；写了就必须是 1..=MAX 的整数
fn timeout_from_args(args: &serde_json::Value) -> anyhow::Result<Duration> {
    let Some(secs) = arg_usize_opt(args, "timeout_secs")? else {
        return Ok(Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    };
    if secs == 0 {
        anyhow::bail!("timeout_secs 必须大于 0");
    }
    let secs = u64::try_from(secs).map_err(|_| anyhow::anyhow!("timeout_secs 太大"))?;
    if secs > MAX_TIMEOUT_SECS {
        anyhow::bail!("timeout_secs 不能超过 {MAX_TIMEOUT_SECS}");
    }
    Ok(Duration::from_secs(secs))
}

// 启动命令并等到它结束、超时或被取消。输出按 max_output_bytes 截断。
// 非 0 退出码仍然是 Ok：模型需要看到 exit 和输出，自己决定要不要改。
async fn run_command(
    root: &Path,
    script: &str,
    timeout: Duration,
    max_output_bytes: usize,
    mut cancel: Option<CancelSignal>,
) -> anyhow::Result<String> {
    // 已经取消了就别启动。否则停止按钮会晚一拍，命令还是跑起来了
    if cancel.as_ref().is_some_and(|signal| signal.is_cancelled()) {
        anyhow::bail!("命令已取消，未启动进程");
    }

    let mut child = prepare_command(script, root)
        .spawn()
        .map_err(|err| anyhow::anyhow!("启动命令失败: {err}"))?;
    // Drop 时杀掉进程组。正常结束会 disarm，避免 pid 被系统复用后误杀
    let mut group = KillGroupOnDrop::arm(child.id());
    // 管道拿出来交给后台任务。不读的话，输出一满子进程就会堵住
    let stdout = child.stdout.take().expect("stdout 已设为 piped");
    let stderr = child.stderr.take().expect("stderr 已设为 piped");
    let stdout_task = tokio::spawn(read_capped(stdout, max_output_bytes));
    let stderr_task = tokio::spawn(read_capped(stderr, max_output_bytes));

    // 取消优先于「刚好跑完」。跑完优先于超时：卡在整点上的成功命令不要报成超时
    let stop = tokio::select! {
        biased;
        _ = wait_for_cancel(&mut cancel) => Stop::Cancelled,
        status = child.wait() => match status {
            Ok(status) => Stop::Exited(status),
            Err(err) => Stop::WaitFailed(err),
        },
        _ = tokio::time::sleep(timeout) => Stop::TimedOut,
    };

    // 不是自己退出的，就要先杀掉再回收。读任务还在，管道不会把 kill 堵住
    if !matches!(stop, Stop::Exited(_)) {
        stop_child(&mut child).await;
    }
    // 进程已经回收，Drop 里不能再按这个 pid 发信号
    group.disarm();

    let stdout = join_output(stdout_task).await;
    let stderr = join_output(stderr_task).await;

    match stop {
        Stop::Exited(status) => {
            let code = status.code().unwrap_or(-1);
            Ok(render_output(&format!("exit: {code}"), &stdout, &stderr))
        }
        Stop::TimedOut => Err(anyhow::anyhow!(
            "{}",
            render_output(
                &format!("命令超时（{} 秒），已终止进程", timeout.as_secs()),
                &stdout,
                &stderr,
            )
        )),
        Stop::Cancelled => Err(anyhow::anyhow!(
            "{}",
            render_output("命令已取消，已终止进程", &stdout, &stderr)
        )),
        Stop::WaitFailed(err) => Err(anyhow::anyhow!(
            "{}",
            render_output(&format!("等待命令失败: {err}"), &stdout, &stderr)
        )),
    }
}

// select 的三个结局。Exited 表示子进程自己结束了，另外三种都要我们去杀
enum Stop {
    Exited(std::process::ExitStatus),
    TimedOut,
    Cancelled,
    WaitFailed(std::io::Error),
}

// 没有取消信号时永远等下去，select 就只会在进程结束或超时上分胜负
async fn wait_for_cancel(cancel: &mut Option<CancelSignal>) {
    if let Some(signal) = cancel {
        // false 表示发送端没了，以后不会再有取消，不能把它当成「用户点了停止」
        if signal.wait_cancelled().await {
            return;
        }
    }
    std::future::pending::<()>().await;
}

// 按当前系统组装命令。cwd 固定为工作区根，标准输入关掉，两个密钥不传给子进程
fn prepare_command(script: &str, root: &Path) -> tokio::process::Command {
    let mut command = shell_command(script);
    command.current_dir(root);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    // 调用方如果中途把 future 丢掉，至少把直接子进程杀掉
    command.kill_on_drop(true);
    // 子进程用得到 PATH，但不该看见 API Key。其它环境变量原样继承
    command.env_remove("DEEPSEEK_API_KEY");
    command.env_remove("MINI_AGENT_API_KEY");
    // 新进程组：取消时可以连 shell 拉起来的 sleep 一起杀
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    command
}

// Unix 用 sh，是因为 CI 和最小环境里不一定把 bash 放在 PATH 上。
// Windows 没有 sh，用 cmd /C。两边的语法不一样，文档里写明了。
fn shell_command(script: &str) -> tokio::process::Command {
    #[cfg(unix)]
    {
        let mut command = tokio::process::Command::new("sh");
        command.arg("-c").arg(script);
        command
    }
    #[cfg(windows)]
    {
        let mut command = tokio::process::Command::new("cmd");
        command.arg("/C").arg(script);
        command
    }
}

// 读到上限后还要继续读，直到 EOF。停手的话管道满了，子进程写不出数据，也就等不到它退出
async fn read_capped(mut reader: impl AsyncRead + Unpin, max_bytes: usize) -> (Vec<u8>, bool) {
    let mut buf = [0u8; 4096];
    let mut collected = Vec::new();
    let mut truncated = false;
    loop {
        let n = match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        if collected.len() >= max_bytes {
            truncated = true;
            continue;
        }
        let room = max_bytes - collected.len();
        if n > room {
            collected.extend_from_slice(&buf[..room]);
            truncated = true;
        } else {
            collected.extend_from_slice(&buf[..n]);
        }
    }
    (collected, truncated)
}

// 读任务正常结束就拿结果。超时或 panic 时不要卡死工具调用，丢弃剩下的输出
async fn join_output(handle: tokio::task::JoinHandle<(Vec<u8>, bool)>) -> (Vec<u8>, bool) {
    match tokio::time::timeout(Duration::from_secs(2), handle).await {
        Ok(Ok(output)) => output,
        _ => (Vec::new(), false),
    }
}

// 杀进程并等它退出。等不到就返回，调用方还有 kill_on_drop 兜底
async fn stop_child(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        kill_process_tree(pid);
    }
    // 再补一刀给直接子进程。进程组杀失败时，这一下仍然能打到 shell
    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
}

// 进程还活着时，Drop 负责把整组杀掉。wait 成功之后必须 disarm。
struct KillGroupOnDrop {
    pid: Option<u32>,
}

impl KillGroupOnDrop {
    fn arm(pid: Option<u32>) -> Self {
        Self { pid }
    }

    fn disarm(&mut self) {
        self.pid = None;
    }
}

impl Drop for KillGroupOnDrop {
    fn drop(&mut self) {
        if let Some(pid) = self.pid.take() {
            kill_process_tree(pid);
        }
    }
}

// Unix：杀进程组，shell 和它拉起的 sleep 都在里面。
// Windows：没有进程组，用 taskkill /T 杀进程树，再由 start_kill 补刀。
fn kill_process_tree(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(unix)]
    {
        let Ok(pid) = i32::try_from(pid) else {
            return;
        };
        let Some(negative) = pid.checked_neg() else {
            return;
        };
        // SAFETY: negative 是刚 spawn 的子进程 pid 的相反数，指向它的进程组。
        // 信号 9 是 SIGKILL，不会被目标进程接住。
        unsafe {
            let _ = kill(negative, 9);
        }
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

// 把退出信息和对两个流的截断说明拼成给模型看的文本
fn render_output(header: &str, stdout: &(Vec<u8>, bool), stderr: &(Vec<u8>, bool)) -> String {
    let mut text = String::new();
    text.push_str(header);
    text.push('\n');
    append_stream(&mut text, "stdout", &stdout.0, stdout.1);
    append_stream(&mut text, "stderr", &stderr.0, stderr.1);
    text
}

// 非 UTF-8 用替换字符显示。截断时先写说明，再写保留下来的正文
fn append_stream(text: &mut String, name: &str, bytes: &[u8], truncated: bool) {
    if truncated {
        text.push_str(&format!("[{name} 已截断，只保留前 {} 字节]\n", bytes.len()));
    }
    text.push_str(&format!("--- {name} ---\n"));
    let body = String::from_utf8_lossy(bytes);
    text.push_str(&body);
    if !body.ends_with('\n') {
        text.push('\n');
    }
}

// 护栏：返回拒绝原因。None 表示可以启动。
// 只按空白和 ; & | 分词，不理解引号，也拦不住 cd 之后才发生的破坏。
fn command_rejected(command: &str) -> Option<&'static str> {
    let compact = compact_command(command);
    // fork bomb 写成好几种空格排版，去掉空白后都看得到这个前缀
    if compact.contains(":(){") {
        return Some("拒绝执行 fork bomb");
    }
    // dd of=/dev/... 以及贴在一起的重定向。>/dev/null 不在这张表里
    if compact.contains("of=/dev/")
        || compact.contains(">/dev/sd")
        || compact.contains(">/dev/nvme")
        || compact.contains(">/dev/hd")
        || compact.contains(">/dev/mmc")
        || compact.contains(">/dev/disk")
    {
        return Some("拒绝把输出写到块设备");
    }
    let tokens = tokenize(command);
    if tokens
        .iter()
        .any(|token| token == "mkfs" || token.starts_with("mkfs."))
    {
        return Some("拒绝执行磁盘格式化（mkfs）");
    }
    if tokens.iter().any(|token| token == "diskpart") {
        return Some("拒绝执行磁盘分区（diskpart）");
    }
    if tokens
        .iter()
        .any(|token| matches!(token.as_str(), "shutdown" | "reboot" | "poweroff" | "halt"))
    {
        return Some("拒绝执行关机或重启");
    }
    if tokens.windows(2).any(|pair| {
        (pair[0] == "init" && matches!(pair[1].as_str(), "0" | "6"))
            || (pair[0] == "systemctl"
                && matches!(
                    pair[1].as_str(),
                    "reboot" | "poweroff" | "halt" | "shutdown"
                ))
    }) {
        return Some("拒绝执行关机或重启");
    }
    if format_targets_drive(&tokens) {
        return Some("拒绝执行磁盘格式化（format）");
    }
    if rm_targets_dangerous(&tokens) {
        return Some("拒绝执行会删掉根目录、家目录或整个工作区的 rm");
    }
    None
}

// 去掉空白和引号，用来抓「没有空格」的写法。引号被丢掉，所以引号里的字也算数
fn compact_command(command: &str) -> String {
    command
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '"' && *ch != '\'')
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

// 在 ; & | 两侧补上空白，这样 `rm -rf /;echo` 不会变成一个词。
// && 和 || 保留成一个词。引号不会被特殊对待。
fn tokenize(command: &str) -> Vec<String> {
    let mut spaced = String::with_capacity(command.len());
    let chars: Vec<char> = command.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == ';' {
            spaced.push(' ');
            spaced.push(';');
            spaced.push(' ');
            index += 1;
            continue;
        }
        if ch == '&' || ch == '|' {
            spaced.push(' ');
            spaced.push(ch);
            if chars.get(index + 1) == Some(&ch) {
                spaced.push(ch);
                index += 1;
            }
            spaced.push(' ');
            index += 1;
            continue;
        }
        spaced.push(ch);
        index += 1;
    }
    spaced
        .split_whitespace()
        .map(normalize_token)
        .filter(|token| !token.is_empty())
        .collect()
}

// 去掉引号并转成小写。大小写不同的 rm / RM 走同一条规则
fn normalize_token(token: &str) -> String {
    token
        .chars()
        .filter(|ch| *ch != '"' && *ch != '\'')
        .collect::<String>()
        .to_ascii_lowercase()
}

fn is_separator(token: &str) -> bool {
    matches!(token, "&&" | "||" | ";" | "|" | "&")
}

// format c: / format C:\ 这类。format 后面直到分隔符为止，出现盘符就拒绝
fn format_targets_drive(tokens: &[String]) -> bool {
    for (index, token) in tokens.iter().enumerate() {
        if token != "format" {
            continue;
        }
        for next in tokens.iter().skip(index + 1) {
            if is_separator(next) {
                break;
            }
            if looks_like_drive(next) {
                return true;
            }
        }
    }
    false
}

// c:、c:\、c:/ 算盘符。后面还跟着别的字符就不是，避免误伤普通单词
fn looks_like_drive(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(letter) = chars.next() else {
        return false;
    };
    if !letter.is_ascii_alphabetic() || chars.next() != Some(':') {
        return false;
    }
    match chars.next() {
        None => true,
        Some('\\' | '/') => chars.next().is_none(),
        Some(_) => false,
    }
}

// 命令行里任何一段 rm，只要又递归又强制，并且目标是根、家目录或整个当前目录，就拒绝。
// 不要求 rm 出现在第一个词，这样 `sudo rm -rf /` 也能被看见。
fn rm_targets_dangerous(tokens: &[String]) -> bool {
    tokens
        .iter()
        .enumerate()
        .any(|(index, token)| is_rm(token) && rm_segment_is_dangerous(&tokens[index + 1..]))
}

fn is_rm(token: &str) -> bool {
    token == "rm"
        || token == "rm.exe"
        || token.ends_with("/rm")
        || token.ends_with("/rm.exe")
        || token.ends_with("\\rm.exe")
}

// 从 rm 的下一个词看到分隔符为止。`--` 之后的词都是路径，不再当选项
fn rm_segment_is_dangerous(tokens: &[String]) -> bool {
    let mut recursive = false;
    let mut force = false;
    let mut targets: Vec<&str> = Vec::new();
    let mut options_ended = false;
    for token in tokens {
        if is_separator(token) {
            break;
        }
        if !options_ended && token == "--" {
            options_ended = true;
            continue;
        }
        if !options_ended && let Some(body) = token.strip_prefix("--") {
            if body == "recursive" {
                recursive = true;
            } else if body == "force" {
                force = true;
            }
            continue;
        }
        if !options_ended && let Some(flags) = token.strip_prefix('-') {
            // -rf/ 这种没有空格的写法：斜杠前面是选项，斜杠开始是路径
            if let Some(slash_at) = flags.find('/') {
                let letters = &flags[..slash_at];
                let path = &flags[slash_at..];
                if letters.contains('r') {
                    recursive = true;
                }
                if letters.contains('f') {
                    force = true;
                }
                if !path.is_empty() {
                    targets.push(path);
                }
            } else {
                if flags.contains('r') {
                    recursive = true;
                }
                if flags.contains('f') {
                    force = true;
                }
            }
            continue;
        }
        targets.push(token.as_str());
    }
    recursive && force && targets.iter().copied().any(dangerous_rm_target)
}

// 这些目标在「递归 + 强制」时会毁掉根、家目录，或者工作区里的全部文件。
// 单独的 `rm -rf target` 不在这里，构建目录要能清。
fn dangerous_rm_target(target: &str) -> bool {
    if !target.is_empty() && target.chars().all(|ch| ch == '/') {
        return true;
    }
    matches!(
        target,
        "/*" | "/."
            | "/.."
            | "~"
            | "~/"
            | "~/*"
            | "$home"
            | "$home/"
            | "$home/*"
            | "${home}"
            | "${home}/"
            | "${home}/*"
            | "."
            | ".."
            | "*"
            | "./*"
    )
}

#[cfg(test)]
mod tests {
    use super::{Bash, command_rejected, run_command};
    use crate::agent::cancel::CancelToken;
    use crate::tools::Tool;
    use crate::tools::sandbox::{TestWorkspace, prepare_root};
    use std::time::{Duration, Instant};

    #[test]
    fn guard_allows_ordinary_commands_and_blocks_dangerous_ones() {
        // false = 放行，true = 拒绝。引号那一行是已知误伤：分词器不理解引号
        let cases = [
            ("echo hello", false),
            ("ls", false),
            ("cargo test", false),
            ("git status", false),
            ("echo hi > /dev/null", false),
            ("rm notes.txt", false),
            ("rm -f notes.txt", false),
            ("rm -f /", false),
            ("rm -r notes.txt", false),
            ("rm -rf target", false),
            ("rm -rf ./target", false),
            ("rm -rf /tmp/foo", false),
            ("rm -rf /", true),
            ("rm -fr /", true),
            ("rm -r -f /", true),
            ("RM -RF /", true),
            ("rm --recursive --force /", true),
            ("rm -rf -- /", true),
            ("rm -rf/", true),
            ("rm -rf /*", true),
            ("rm -rf ~", true),
            ("rm -rf $HOME", true),
            ("rm -rf .", true),
            ("rm -rf ..", true),
            ("rm -rf *", true),
            ("rm -rf ./*", true),
            ("sudo rm -rf /", true),
            ("/bin/rm -rf /", true),
            ("rm -rf /;echo hi", true),
            ("rm -rf \"/\"", true),
            ("echo \"rm -rf /\"", true),
            ("mkfs.ext4 /dev/sda", true),
            ("dd if=/dev/zero of=/dev/sda", true),
            ("dd if=/dev/zero of=/dev/null", true),
            ("echo hi>/dev/sda", true),
            (":(){ :|:& };:", true),
            ("shutdown -h now", true),
            ("reboot", true),
            ("poweroff", true),
            ("halt", true),
            ("init 0", true),
            ("systemctl reboot", true),
            ("diskpart", true),
            ("format c:", true),
            ("format C:\\", true),
        ];
        for (command, blocked) in cases {
            assert_eq!(
                command_rejected(command).is_some(),
                blocked,
                "护栏判断和预期不一致: {command}"
            );
        }
        let reason = command_rejected("rm -rf /").expect("应该拒绝");
        assert!(reason.contains("rm"), "{reason}");
    }

    #[tokio::test]
    async fn rejects_bad_arguments_without_spawning() {
        let ws = TestWorkspace::new();
        let tool = Bash::new(ws.path(), None);
        let empty = tool
            .execute(serde_json::json!({"command": "   "}))
            .await
            .unwrap_err();
        assert!(empty.to_string().contains("不能为空"));
        let missing = tool.execute(serde_json::json!({})).await.unwrap_err();
        assert!(missing.to_string().contains("command"));
        let zero = tool
            .execute(serde_json::json!({"command": "echo hi", "timeout_secs": 0}))
            .await
            .unwrap_err();
        assert!(zero.to_string().contains("大于 0"));
        let huge = tool
            .execute(serde_json::json!({"command": "echo hi", "timeout_secs": 121}))
            .await
            .unwrap_err();
        assert!(huge.to_string().contains("不能超过"));
        let negative = tool
            .execute(serde_json::json!({"command": "echo hi", "timeout_secs": -1}))
            .await
            .unwrap_err();
        assert!(negative.to_string().contains("负数"));
    }

    #[tokio::test]
    async fn rejects_dangerous_command_without_spawning() {
        let ws = TestWorkspace::new();
        let tool = Bash::new(ws.path(), None);
        let err = tool
            .execute(serde_json::json!({"command": "rm -rf / && touch pwned"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("拒绝"), "{err}");
        assert!(!ws.path().join("pwned").exists(), "被拒绝的命令不应该启动");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn runs_in_workspace_and_reports_exit_code() {
        let ws = TestWorkspace::new();
        let root = prepare_root(ws.path()).expect("临时工作区应该存在");
        let tool = Bash::new(ws.path(), None);
        let out = tool
            .execute(serde_json::json!({
                "command": "pwd; echo hi; echo bad >&2; exit 3"
            }))
            .await
            .expect("非 0 退出码也应该把输出交回去");
        assert!(out.contains("exit: 3"), "{out}");
        assert!(out.contains(&root.display().to_string()), "{out}");
        assert!(out.contains("hi"), "{out}");
        assert!(out.contains("bad"), "{out}");
        assert!(out.contains("--- stdout ---"), "{out}");
        assert!(out.contains("--- stderr ---"), "{out}");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn timeout_kills_the_child() {
        let ws = TestWorkspace::new();
        let tool = Bash::new(ws.path(), None);
        let started = Instant::now();
        let err = tool
            .execute(serde_json::json!({"command": "sleep 5", "timeout_secs": 1}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("超时"), "{err}");
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "超时后应该马上返回，实际 {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn cancel_kills_before_the_side_effect() {
        let ws = TestWorkspace::new();
        let token = CancelToken::new();
        let tool = Bash::new(ws.path(), Some(token.subscribe()));
        let trigger = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            trigger.cancel();
        });
        let started = Instant::now();
        let err = tool
            .execute(serde_json::json!({
                "command": "sleep 2 && touch leaked.txt",
                "timeout_secs": 30
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("取消"), "{err}");
        let rest = Duration::from_secs(3).saturating_sub(started.elapsed());
        tokio::time::sleep(rest).await;
        assert!(!ws.path().join("leaked.txt").exists(), "取消后子进程还在跑");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn already_cancelled_does_not_start() {
        let ws = TestWorkspace::new();
        let token = CancelToken::new();
        token.cancel();
        let tool = Bash::new(ws.path(), Some(token.subscribe()));
        let started = Instant::now();
        let err = tool
            .execute(serde_json::json!({
                "command": "sleep 5 && touch leaked.txt",
                "timeout_secs": 30
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("未启动"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(!ws.path().join("leaked.txt").exists());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn truncates_output_without_deadlocking() {
        let ws = TestWorkspace::new();
        let root = prepare_root(ws.path()).expect("临时工作区应该存在");
        let small = run_command(&root, "echo 0123456789", Duration::from_secs(5), 4, None)
            .await
            .expect("短输出应该成功");
        assert!(
            small.contains("[stdout 已截断，只保留前 4 字节]"),
            "{small}"
        );
        assert!(small.contains("0123"), "{small}");
        assert!(!small.contains("456789"), "{small}");

        // 256 KiB 大于常见的管道缓冲。截断之后如果不再读，子进程会写满管道，这个 5 秒超时就会失败
        let started = Instant::now();
        let large = run_command(
            &root,
            "dd if=/dev/zero bs=1024 count=256 2>/dev/null",
            Duration::from_secs(5),
            8,
            None,
        )
        .await
        .expect("大量输出也应该在超时前结束");
        assert!(large.contains("exit: 0"), "{large}");
        assert!(
            large.contains("[stdout 已截断，只保留前 8 字节]"),
            "{large}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "读管道被堵住了，实际 {:?}",
            started.elapsed()
        );
    }
}
