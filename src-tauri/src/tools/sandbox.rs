// ============================================================================
// 工作区沙箱
//
// 文件工具只能碰「工作区根目录」里面的路径。模型传来的 path 可能是：
//   - 相对路径：notes/a.txt
//   - 带着 .. 的相对路径：../../etc/passwd
//   - 绝对路径：/etc/passwd
//   - 指向外部的符号链接
//
// 处理顺序：
//   1. 把根目录 canonicalize，变成没有 .. 、也没有符号链接的绝对路径
//   2. 把用户路径按词法归一化（这一步不访问磁盘），先挡住 .. 逃逸
//   3. 目标已经存在时再 canonicalize 一次，挡住「链接指向外部」
//
// 这一层不做的事：不隐藏工作区内部的密钥。根目录里如果有 .env，read 仍然读得到。
// ============================================================================

// 词法归一化要按路径分量处理，不能当成字符串前缀比较
use std::io::Write;
use std::path::{Component, Path, PathBuf};
// 临时文件名用进程内递增序号，避免依赖额外的随机数 crate
use std::sync::atomic::{AtomicU64, Ordering};

// 环境变量名：不设就用进程当前目录。AppState 启动时读一次。
pub const WORKSPACE_ENV: &str = "MINI_AGENT_WORKSPACE";

// 从环境变量取出工作区根。空字符串当成「没设置」。
pub fn workspace_root_from_env() -> PathBuf {
    // var 失败说明变量不存在；存在但空白也退回默认值
    match std::env::var(WORKSPACE_ENV) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                default_workspace_root()
            } else {
                PathBuf::from(trimmed)
            }
        }
        Err(_) => default_workspace_root(),
    }
}

// 启动时调用：能规范化就存绝对路径，避免之后工作目录变了根也跟着漂。
// 目录暂时不存在时保留原值，真正用到工具时再报「根目录不存在」。
pub fn load_workspace_root() -> PathBuf {
    let raw = workspace_root_from_env();
    prepare_root(&raw).unwrap_or(raw)
}

// 当前目录拿不到时退回 "."，后面的 canonicalize 会给出明确错误。
fn default_workspace_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

// 把根目录变成真实绝对路径。相对路径按「此刻的当前目录」拼接。
pub fn prepare_root(root: &Path) -> anyhow::Result<PathBuf> {
    if root.as_os_str().is_empty() {
        anyhow::bail!("工作区根目录不能为空");
    }
    // 相对根要先变绝对，否则后面和绝对路径比「是不是在里面」没有意义
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| anyhow::anyhow!("无法取得当前目录: {err}"))?
            .join(root)
    };
    absolute.canonicalize().map_err(|err| {
        anyhow::anyhow!("工作区根目录不存在或无法访问 {}: {err}", absolute.display())
    })
}

// 把用户给的路径解析成工作区内部的绝对路径。逃出去就返回错误，不产生文件。
pub fn resolve_inside(root: &Path, user_path: &str) -> anyhow::Result<PathBuf> {
    let root = prepare_root(root)?;
    let user_path = user_path.trim();
    if user_path.is_empty() {
        anyhow::bail!("路径不能为空");
    }
    // NUL 会把 C 字符串截断；Rust 的 Path 能装下它，但没有合法用途
    if user_path.contains('\0') {
        anyhow::bail!("路径含有非法字符");
    }
    let raw = Path::new(user_path);
    // 绝对路径不再拼接，但仍然必须落在根里面
    let joined = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        root.join(raw)
    };
    let lexical = normalize_lexical(&joined);
    if !is_inside(&root, &lexical) {
        anyhow::bail!("路径逃出工作区: {user_path}");
    }
    // 已经存在的路径用真实路径再验一次，符号链接这一步才会现形
    if lexical.exists() {
        let canonical = lexical
            .canonicalize()
            .map_err(|err| io_err("解析", &lexical, err))?;
        if !is_inside(&root, &canonical) {
            anyhow::bail!("路径逃出工作区: {user_path}");
        }
        return Ok(canonical);
    }
    // "." 归一化后就是根；根没有「文件名」这一分量
    if lexical == root {
        return Ok(root);
    }
    let Some(file_name) = lexical.file_name() else {
        anyhow::bail!("无法解析路径: {user_path}");
    };
    let parent = lexical.parent().unwrap_or(Path::new(""));
    // 新文件的父目录必须真实落在工作区内，避免「父目录是指向外部的链接」
    let parent_inside = canonical_parent_inside(&root, parent, user_path)?;
    let full = parent_inside.join(file_name);
    if !is_inside(&root, &full) {
        anyhow::bail!("路径逃出工作区: {user_path}");
    }
    Ok(full)
}

// 父目录可能还不存在（write 到没建过的子目录）。找到最近的、已经存在的祖先再比对。
fn canonical_parent_inside(root: &Path, parent: &Path, user_path: &str) -> anyhow::Result<PathBuf> {
    if parent.as_os_str().is_empty() {
        anyhow::bail!("无法确定父目录: {user_path}");
    }
    if parent.exists() {
        let canonical = parent
            .canonicalize()
            .map_err(|err| io_err("解析", parent, err))?;
        if !is_inside(root, &canonical) {
            anyhow::bail!("路径逃出工作区: {user_path}");
        }
        return Ok(canonical);
    }
    let ancestor = nearest_existing_ancestor(parent)?;
    let ancestor_canon = ancestor
        .canonicalize()
        .map_err(|err| io_err("解析", ancestor, err))?;
    if !is_inside(root, &ancestor_canon) {
        anyhow::bail!("路径逃出工作区: {user_path}");
    }
    // 祖先之后的那段还没落盘，接回真实祖先上，再检查一次前缀
    let rest = parent.strip_prefix(ancestor).unwrap_or(Path::new(""));
    let combined = ancestor_canon.join(rest);
    if !is_inside(root, &combined) {
        anyhow::bail!("路径逃出工作区: {user_path}");
    }
    Ok(combined)
}

// 沿 parent 往上走，直到碰到一个真实存在的目录。根目录 `/` 一定存在。
fn nearest_existing_ancestor(mut path: &Path) -> anyhow::Result<&Path> {
    loop {
        if path.exists() {
            return Ok(path);
        }
        match path.parent() {
            Some(parent) if parent != path => path = parent,
            _ => anyhow::bail!("父目录不存在: {}", path.display()),
        }
    }
}

// 去掉 `.` 和能抵消的 `..`。绝对路径走到根之后，多余的 `..` 就停在根上。
fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                out.push(prefix.as_os_str());
            }
            Component::RootDir => {
                out.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                // 只弹出普通目录名。根和盘符留着，避免把 `C:\` 弹没
                if !pop_normal(&mut out) && !out.has_root() {
                    out.push("..");
                }
            }
            Component::Normal(segment) => {
                out.push(segment);
            }
        }
    }
    out
}

// 最后一个分量是普通名字才弹出。返回 false 表示已经到了根、或者只剩 `..`。
fn pop_normal(path: &mut PathBuf) -> bool {
    if matches!(path.components().next_back(), Some(Component::Normal(_))) {
        path.pop();
        true
    } else {
        false
    }
}

// `Path::starts_with` 按分量比较：`/ws` 包住 `/ws/a`，但包不住 `/ws-secret`。
fn is_inside(root: &Path, candidate: &Path) -> bool {
    candidate == root || candidate.starts_with(root)
}

// 读一个不大的 UTF-8 文本。目录、二进制、超限都在这里拒绝，调用方不用再判一遍。
pub fn read_utf8_file(path: &Path, max_bytes: u64) -> anyhow::Result<String> {
    let meta = std::fs::metadata(path).map_err(|err| io_err("读取", path, err))?;
    if meta.is_dir() {
        anyhow::bail!("路径是目录，不是文件: {}", path.display());
    }
    if meta.len() > max_bytes {
        anyhow::bail!("文件过大（{} 字节），超过上限 {max_bytes}", meta.len());
    }
    let bytes = std::fs::read(path).map_err(|err| io_err("读取", path, err))?;
    // NUL 是最常见的二进制标记；UTF-8 文本里不该出现
    if bytes.contains(&0) {
        anyhow::bail!("只支持文本文件，检测到二进制内容: {}", path.display());
    }
    String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("只支持 UTF-8 文本: {}", path.display()))
}

// 先写同目录临时文件，再 rename 覆盖目标。同一目录里的 rename 在 POSIX 上是原子的。
pub fn atomic_write(path: &Path, contents: &str) -> anyhow::Result<()> {
    if path.is_dir() {
        anyhow::bail!("路径是目录，不是文件: {}", path.display());
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| anyhow::anyhow!("无法确定父目录: {}", path.display()))?;
    // 不自动 mkdir：模型少写一级目录时，宁可报错也不要悄悄造出一串文件夹
    if !parent.is_dir() {
        anyhow::bail!("父目录不存在: {}", parent.display());
    }
    let tmp_path = temp_file_path(parent);
    if let Err(err) = write_temp(&tmp_path, contents) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }
    if let Err(err) = replace_file(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }
    Ok(())
}

// 临时文件和目标放在同一目录，rename 才不会跨文件系统变成「复制」。
fn temp_file_path(parent: &Path) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(".mini-agent-tmp-{}-{n}", std::process::id()))
}

fn write_temp(tmp_path: &Path, contents: &str) -> anyhow::Result<()> {
    let mut file =
        std::fs::File::create(tmp_path).map_err(|err| io_err("创建临时文件", tmp_path, err))?;
    file.write_all(contents.as_bytes())
        .map_err(|err| io_err("写入临时文件", tmp_path, err))?;
    // 刷到磁盘再改名，避免掉电后留下空文件顶替旧内容
    file.sync_all()
        .map_err(|err| io_err("刷盘", tmp_path, err))?;
    Ok(())
}

fn replace_file(tmp_path: &Path, dest: &Path) -> anyhow::Result<()> {
    // Windows 上目标已存在时 rename 会失败，只能先删再改名，中间有极短的空窗。
    #[cfg(windows)]
    if dest.exists() {
        std::fs::remove_file(dest).map_err(|err| io_err("替换前删除旧文件", dest, err))?;
    }
    std::fs::rename(tmp_path, dest).map_err(|err| io_err("替换", dest, err))
}

// IO 错误翻成中文。找不到和其它失败分开，模型才知道该不该换路径。
fn io_err(action: &str, path: &Path, err: std::io::Error) -> anyhow::Error {
    if err.kind() == std::io::ErrorKind::NotFound {
        anyhow::anyhow!("{action}失败，找不到路径: {}", path.display())
    } else {
        anyhow::anyhow!("{action}失败 {}: {err}", path.display())
    }
}

// ============================================================================
// 参数解析：四个文件工具共用，避免每个文件各写一遍「JSON 里取出字符串」
// ============================================================================

// 路径类参数：去掉首尾空白，空白字符串视为没传。
pub fn arg_path(args: &serde_json::Value, key: &str) -> anyhow::Result<String> {
    match args.get(key) {
        Some(serde_json::Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                anyhow::bail!("参数 {key} 不能为空");
            }
            Ok(trimmed.to_string())
        }
        _ => anyhow::bail!("缺少字符串参数 {key}"),
    }
}

// 正文类参数：空字符串是合法值（清空文件、把一段文字删掉），不要 trim。
pub fn arg_string(args: &serde_json::Value, key: &str) -> anyhow::Result<String> {
    match args.get(key) {
        Some(serde_json::Value::String(text)) => Ok(text.clone()),
        _ => anyhow::bail!("缺少字符串参数 {key}"),
    }
}

// list_dir 的 path 可以省略，省略时用 default（一般是 "."）。
pub fn arg_path_or(args: &serde_json::Value, key: &str, default: &str) -> anyhow::Result<String> {
    match args.get(key) {
        None | Some(serde_json::Value::Null) => Ok(default.to_string()),
        Some(serde_json::Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Ok(default.to_string())
            } else {
                Ok(trimmed.to_string())
            }
        }
        Some(_) => anyhow::bail!("参数 {key} 必须是字符串"),
    }
}

// offset / limit。缺省是 None，由工具自己填默认值。负数和小数直接报错。
pub fn arg_usize_opt(args: &serde_json::Value, key: &str) -> anyhow::Result<Option<usize>> {
    match args.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => Ok(Some(json_usize(value, key)?)),
    }
}

fn json_usize(value: &serde_json::Value, key: &str) -> anyhow::Result<usize> {
    if let Some(n) = value.as_u64() {
        return usize::try_from(n).map_err(|_| anyhow::anyhow!("{key} 太大"));
    }
    if let Some(n) = value.as_i64() {
        if n < 0 {
            anyhow::bail!("{key} 不能是负数");
        }
        return usize::try_from(n as u64).map_err(|_| anyhow::anyhow!("{key} 太大"));
    }
    // 有的模型会写 1.0。只接受恰好是整数的有限数。
    if let Some(n) = value.as_f64()
        && n.is_finite()
        && n >= 0.0
    {
        let as_u = n as u64;
        if as_u as f64 == n {
            return usize::try_from(as_u).map_err(|_| anyhow::anyhow!("{key} 太大"));
        }
    }
    anyhow::bail!("{key} 必须是非负整数")
}

// 测试用的临时工作区。放在这个模块里，是因为文件工具和 bash 的测试都要一个真实目录。
#[cfg(test)]
pub(crate) struct TestWorkspace {
    path: PathBuf,
}

#[cfg(test)]
impl TestWorkspace {
    pub(crate) fn new() -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("mini-agent-ws-{nanos}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("应该能创建临时工作区");
        Self { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    // 写入相对路径。中间缺的目录会建出来，专门给「准备夹具」用，正式的 write 工具不会这么做。
    pub(crate) fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let path = self.path.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("应该能创建夹具目录");
        }
        std::fs::write(&path, contents).expect("应该能写入夹具文件");
        path
    }
}

#[cfg(test)]
impl Drop for TestWorkspace {
    fn drop(&mut self) {
        // 只删我们自己造出来的子目录，避免 path 算错时把整个临时目录清掉
        let temp = std::env::temp_dir();
        if self.path.starts_with(&temp) && self.path != temp {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_and_dot_resolve_inside_root() {
        let ws = TestWorkspace::new();
        let file = ws.write("a.txt", "hi");
        let resolved = resolve_inside(ws.path(), "a.txt").expect("相对路径应该成功");
        assert_eq!(resolved, file.canonicalize().unwrap());
        let dot = resolve_inside(ws.path(), ".").expect("点应该就是根");
        assert_eq!(dot, ws.path().canonicalize().unwrap());
        // 绝对路径只要真的在根里面，也允许
        let abs = resolved.to_string_lossy().to_string();
        let via_abs = resolve_inside(ws.path(), &abs).expect("根内绝对路径应该成功");
        assert_eq!(via_abs, resolved);
    }

    #[test]
    fn dot_dot_staying_inside_is_allowed() {
        let ws = TestWorkspace::new();
        ws.write("sub/nested.txt", "nested");
        ws.write("a.txt", "root");
        let resolved = resolve_inside(ws.path(), "sub/../a.txt").expect("抵消后的 .. 应该留下");
        assert_eq!(std::fs::read_to_string(resolved).unwrap(), "root");
    }

    #[test]
    fn escapes_are_rejected() {
        let ws = TestWorkspace::new();
        ws.write("a.txt", "hi");
        for bad in [
            "../a.txt",
            "/etc/passwd",
            "",
            "   ",
            "a\0b",
            "sub/../../outside.txt",
        ] {
            let err = resolve_inside(ws.path(), bad).expect_err("逃逸路径应该失败");
            let message = err.to_string();
            assert!(
                message.contains("逃出")
                    || message.contains("不能为空")
                    || message.contains("非法"),
                "{bad} -> {message}"
            );
        }
    }

    #[test]
    fn sibling_prefix_is_not_inside() {
        // `/tmp/ws` 不能因为字符串前缀关系吃掉 `/tmp/ws-secret`
        let base = TestWorkspace::new();
        let root = base.path().join("ws");
        let secret_dir = base.path().join("ws-secret");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&secret_dir).unwrap();
        std::fs::write(secret_dir.join("secret.txt"), "topsecret").unwrap();
        let err = resolve_inside(&root, "../ws-secret/secret.txt").unwrap_err();
        assert!(err.to_string().contains("逃出"), "{err}");
        let abs = secret_dir.join("secret.txt");
        let err = resolve_inside(&root, &abs.to_string_lossy()).unwrap_err();
        assert!(err.to_string().contains("逃出"), "{err}");
        assert_eq!(
            std::fs::read_to_string(secret_dir.join("secret.txt")).unwrap(),
            "topsecret"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_rejected() {
        let ws = TestWorkspace::new();
        let outside = TestWorkspace::new();
        let secret = outside.write("secret.txt", "nope");
        std::os::unix::fs::symlink(&secret, ws.path().join("leak")).unwrap();
        let err = resolve_inside(ws.path(), "leak").unwrap_err();
        assert!(err.to_string().contains("逃出"), "{err}");
        // 父目录本身是指向外部的链接时，新文件也不能跟着写出去
        std::os::unix::fs::symlink(outside.path(), ws.path().join("out")).unwrap();
        let err = resolve_inside(ws.path(), "out/pwned.txt").unwrap_err();
        assert!(err.to_string().contains("逃出"), "{err}");
        assert!(!outside.path().join("pwned.txt").exists());
    }

    #[test]
    fn missing_workspace_root_errors() {
        let missing = std::env::temp_dir().join(format!(
            "mini-agent-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let err = resolve_inside(&missing, "a.txt").unwrap_err();
        assert!(err.to_string().contains("不存在"), "{err}");
    }

    #[test]
    fn atomic_write_replaces_without_leaving_temp() {
        let ws = TestWorkspace::new();
        let file = ws.path().join("a.txt");
        atomic_write(&file, "你好").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "你好");
        atomic_write(&file, "two").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
        let names: Vec<_> = std::fs::read_dir(ws.path())
            .unwrap()
            .map(|ent| ent.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names
                .iter()
                .all(|name| !name.starts_with(".mini-agent-tmp-")),
            "{names:?}"
        );
        let err = atomic_write(&ws.path().join("nope").join("a.txt"), "x").unwrap_err();
        assert!(err.to_string().contains("父目录"), "{err}");
        assert!(!ws.path().join("nope").exists());
    }
}
