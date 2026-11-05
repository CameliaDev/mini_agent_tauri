// 声明 time 子模块，Rust 会去加载 src/tools/builtin/time.rs
pub mod time;
// 声明 echo 子模块，Rust 会去加载 src/tools/builtin/echo.rs
pub mod echo;
// 声明 read 子模块：按路径读取工作区内的文本
pub mod read;
// 声明 write 子模块：原子写入工作区内的文件
pub mod write;
// 声明 edit 子模块：唯一匹配替换
pub mod edit;
// 声明 list_dir 子模块：列出一层目录
pub mod list_dir;
// 声明 bash 子模块：在工作区里执行命令
pub mod bash;
