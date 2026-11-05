// App：两栏布局 —— 左侧会话栏 + 右侧对话区
import { useEffect, useState } from "react";
import { Composer } from "./components/Composer";
import { MessageList } from "./components/MessageList";
import { Sidebar } from "./components/Sidebar";
import { activeMessages, useChatStore } from "./store";
// 引入持久化层的启动函数、清空函数和落库函数
import { clearMessages, hydrateFromDb, persistSession } from "./ipc/sessions";
import "./App.css";

// 窄屏断点：必须和 App.css 里 @media (max-width: 860px) 保持一致。
// JS 判断「是不是抽屉模式」、CSS 决定「怎么摆」用的是同一个数字，改的时候两处一起改。
const NARROW_QUERY = "(max-width: 860px)";

function App() {
  const error = useChatStore((s) => s.error);
  const clearError = useChatStore((s) => s.clearError);
  const clearActiveSession = useChatStore((s) => s.clearActiveSession);
  // 当前会话里的消息条数，用来决定「清空对话」按钮是否可点
  const messageCount = useChatStore((s) => activeMessages(s).length);
  // 窄屏时侧栏是浮在主区上面的抽屉，默认必须收起，
  // 否则它一进窄屏就盖住顶栏、消息和输入框（截图里的现象）
  const [sidebarOpen, setSidebarOpen] = useState(
    () => !window.matchMedia(NARROW_QUERY).matches,
  );

  // 跟随窗口宽度变化：宽屏常驻展开、窄屏自动收起。
  // 用 matchMedia 的 change 事件，比监听 resize 更准（只在跨过断点时触发一次）
  useEffect(() => {
    const mql = window.matchMedia(NARROW_QUERY);
    const onChange = (event: MediaQueryListEvent) => {
      setSidebarOpen(!event.matches);
    };
    mql.addEventListener("change", onChange);
    // 卸载时摘掉监听，避免重复注册
    return () => mql.removeEventListener("change", onChange);
  }, []);

  // 启动时从数据库把会话读回来
  // useEffect 的第二个参数 [] 表示只在挂载时跑一次
  useEffect(() => {
    // 读不到就忽略：数据库是空的也能正常用
    void hydrateFromDb().catch(() => undefined);
  }, []);

  // 清空当前对话：内存里清，数据库里也清
  async function clearChat() {
    // 记住当前会话 id
    const sessionId = useChatStore.getState().activeSessionId;
    // 先清数据库里的消息
    await clearMessages(sessionId);
    // 再清内存
    clearActiveSession();
    // 最后把会话元信息落库：标题会重置成「新对话」
    await persistSession(sessionId);
  }

  return (
    <div className={`app${sidebarOpen ? "" : " app--collapsed"}`}>
      {/* 窄屏下侧栏是浮在上面的，这里加一层遮罩，点它就收起侧栏 */}
      {sidebarOpen && (
        <div
          className="scrim"
          onClick={() => setSidebarOpen(false)}
          aria-hidden="true"
        />
      )}
      <Sidebar />

      <main className="main">
        <header className="topbar">
          <button
            type="button"
            className="topbar__toggle"
            onClick={() => setSidebarOpen((v) => !v)}
            aria-label="切换会话栏"
          >
            ☰
          </button>
          <div className="topbar__meta">
            <span className="topbar__name">mini_agent</span>
            <span className="topbar__model">DeepSeek · deepseek-chat</span>
          </div>
          <button
            type="button"
            className="topbar__action"
            onClick={() => void clearChat()}
            disabled={messageCount === 0}
          >
            清空对话
          </button>
        </header>

        {error && (
          <div className="banner" role="alert">
            <span className="banner__text">{error}</span>
            <button type="button" className="banner__close" onClick={clearError}>
              知道了
            </button>
          </div>
        )}

        <MessageList />
        <Composer />
      </main>
    </div>
  );
}

export default App;
