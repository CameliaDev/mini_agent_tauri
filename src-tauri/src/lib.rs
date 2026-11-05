// 声明 agent 模块，Rust 会去加载 src/agent/mod.rs
pub mod agent;
// 声明 app 模块，Rust 会去加载 src/app/mod.rs
pub mod app;
// 声明 protocol 模块，Rust 会去加载 src/protocol.rs
pub mod protocol;
// 声明 providers 模块，Rust 会去加载 src/providers/mod.rs
pub mod providers;
// 声明 tools 模块，Rust 会去加载 src/tools/mod.rs
pub mod tools;
// 声明 storage 模块，Rust 会去加载 src/storage/mod.rs
pub mod storage;

// 引入要注册的 Tauri 命令，generate_handler 里要用
use app::commands::{
    append_message, chat, clear_session_messages, delete_session, delete_session_messages, greet,
    list_sessions, load_messages, save_session, stop_generation,
};
// 引入应用共享状态
use app::state::AppState;
// 引入存储层
use storage::Storage;
// 引入 Tauri 的 Manager trait：app.path() 和 app.manage() 都来自它
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // 把配置注册成全局状态，command 里通过 State 取用
        .manage(AppState::from_env())
        // setup 在窗口创建前跑一次，这里把数据库连接准备好
        .setup(|app| {
            // 取应用数据目录（Windows 是 %APPDATA%/<应用名>）
            let dir = app.path().app_data_dir()?;
            // 数据库文件名固定
            let db_path = dir.join("mini_agent.db");
            // 打开数据库（阻塞操作，但 setup 阶段没有别的任务在跑，可以接受）
            let storage = tauri::async_runtime::block_on(Storage::open(db_path))?;
            // 注册成全局状态，command 里通过 State 取用
            app.manage(storage);
            // setup 返回 Ok 表示成功
            Ok(())
        })
        // 注册所有命令：模板的 greet、聊天的 chat、会话持久化的几个
        .invoke_handler(tauri::generate_handler![
            greet,
            chat,
            stop_generation,
            list_sessions,
            load_messages,
            save_session,
            append_message,
            delete_session,
            delete_session_messages,
            clear_session_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
