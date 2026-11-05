// App：两栏布局 —— 左侧会话栏 + 右侧对话区
import { useEffect, useRef, useState } from "react";
import { Composer } from "./components/Composer";
import { MessageList } from "./components/MessageList";
import { Sidebar } from "./components/Sidebar";
import { useSidebarWidth } from "./hooks/useSidebarWidth";
import { activeMessages, useChatStore } from "./store";
// 引入持久化层的启动函数、清空函数和落库函数
import { clearMessages, hydrateFromDb, persistSession } from "./ipc/sessions";
import "./App.css";

// 抽屉断点：必须和 App.css 里 @media (max-width: 640px) 保持一致。
// JS 判断「是不是抽屉模式」、CSS 决定「怎么摆」用的是同一个数字，改的时候两处一起改。
// 默认窗口是 800×600，这个宽度以及更宽的桌面都要并排推挤，不能盖住对话。
// 只有 ≤640px 才改成浮层：248px 侧栏再往里挤，主栏会窄到顶栏和输入区排不下。
// 并排时侧栏可以拖宽，但抽屉自己的宽度仍是 248px，不跟着已保存的宽度变。
const NARROW_QUERY = "(max-width: 640px)";

function App() {
  const error = useChatStore((s) => s.error);
  const clearError = useChatStore((s) => s.clearError);
  const clearActiveSession = useChatStore((s) => s.clearActiveSession);
  // 当前会话里的消息条数，用来决定「清空对话」按钮是否可点
  const messageCount = useChatStore((s) => activeMessages(s).length);
  // 极窄时侧栏是浮在主区上面的抽屉，默认必须收起，
  // 否则一进页面就盖住顶栏、消息和输入框。桌面宽度默认展开，走并排推挤。
  const [sidebarOpen, setSidebarOpen] = useState(
    () => !window.matchMedia(NARROW_QUERY).matches,
  );
  // 宽度写在 .app 的 CSS 变量上，侧栏和收起用的负 margin 读同一个数
  const appRef = useRef<HTMLDivElement>(null);
  const sidebarWidth = useSidebarWidth(appRef);

  // 跟随窗口宽度变化：桌面宽度展开并排、极窄自动收起。
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
    // 壳层就是侧栏 + 主栏。侧栏打开时主栏被挤到旁边，对话不会被盖住。
    // 以后设置页可以放进主栏，会话列表仍留在左侧。
    <div
      ref={appRef}
      className={`app${sidebarOpen ? "" : " app--collapsed"}`}
    >
      {/* 极窄时侧栏才浮在上面，这时加一层遮罩，点它就收起侧栏 */}
      {sidebarOpen && (
        <div
          className="scrim"
          onClick={() => setSidebarOpen(false)}
          aria-hidden="true"
        />
      )}
      <Sidebar />
      {/* 贴在侧栏右缘上，本身不占宽度。≤640px 抽屉和收起时由 CSS 藏掉。
          不要给它加 transform / position:fixed，输入框的祖先上也不能有。 */}
      <div
        className="sidebar-resizer"
        role="separator"
        aria-orientation="vertical"
        aria-label="调整会话栏宽度"
        aria-valuemin={sidebarWidth.min}
        aria-valuemax={sidebarWidth.max}
        aria-valuenow={sidebarWidth.width}
        aria-valuetext={`${sidebarWidth.width}像素`}
        tabIndex={0}
        onPointerDown={sidebarWidth.onPointerDown}
        onKeyDown={sidebarWidth.onKeyDown}
      />

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
