// 左侧会话栏：新建对话 + 会话列表（按今天 / 昨天 / 更早分组）
// 底部是本地用户和设置入口。没有账号系统，设置只展示模型、工作区和密钥该写在哪
import { useEffect, useMemo, useState } from "react";
import { groupOf, useChatStore } from "../store";
import type { Session } from "../store";
// 引入持久化层的三个函数：切换时读消息、删除时删库、新建时落库
import { loadMessagesOf, persistSession, removeSession } from "../ipc/sessions";

export function Sidebar() {
  const sessions = useChatStore((s) => s.sessions);
  const activeSessionId = useChatStore((s) => s.activeSessionId);
  const newSession = useChatStore((s) => s.newSession);
  const selectSession = useChatStore((s) => s.selectSession);
  const deleteSession = useChatStore((s) => s.deleteSession);
  // 设置面板开着时盖在会话列表下沿，不另开窗口
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    if (!settingsOpen) return;
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") setSettingsOpen(false);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [settingsOpen]);

  // 切换会话：先切内存，离开的那份对话立刻排队落库，目标会话只在还没灌过时才读库
  async function openSession(id: string) {
    const previousId = useChatStore.getState().activeSessionId;
    if (previousId !== id) {
      // 内存里立刻切过去，界面马上响应
      selectSession(id);
      // 切走前把离开的会话写进库。生成还没结束时消息只在内存里，
      // 不写的话，下次启动就丢了。写入失败不拦住切换，内存里这份还在。
      void persistSession(previousId).catch(() => undefined);
    }
    // 目标会话若已经在内存里接管，load 会直接返回，不会用旧快照盖掉
    await loadMessagesOf(id);
  }

  // 删除会话：内存里删，数据库里也删
  async function remove(id: string) {
    // 先删数据库那边的（消息随外键级联一起删）
    await removeSession(id);
    // 再删内存里的
    deleteSession(id);
  }

  // 新建对话：旧会话里还没落库的消息先写回去，再建这个空会话
  async function create() {
    const previousId = useChatStore.getState().activeSessionId;
    // 内存里新建（会切过去）。旧会话的消息还留在它自己的列表里
    newSession();
    const createdId = useChatStore.getState().activeSessionId;
    // 新会话也要落库。旧会话写入失败时不能把它漏掉。
    try {
      await persistSession(previousId);
    } finally {
      await persistSession(createdId);
    }
  }

  // 分组：先按时间分桶，桶内保持原有顺序（新的在前）
  const groups = useMemo(() => {
    const map = new Map<string, Session[]>();
    for (const session of sessions) {
      const key = groupOf(session);
      map.set(key, [...(map.get(key) ?? []), session]);
    }
    return [...map.entries()];
  }, [sessions]);

  return (
    <aside className="sidebar">
      <div className="sidebar__head">
        <button type="button" className="sidebar__new" onClick={() => void create()}>
          <span className="sidebar__plus" aria-hidden="true">
            +
          </span>
          新建对话
        </button>
      </div>

      <nav className="sidebar__list">
        {groups.map(([label, items]) => (
          <section key={label} className="sidebar__group">
            <h2 className="sidebar__group-title">{label}</h2>
            {items.map((session) => (
              <div
                key={session.id}
                className={`sidebar__row${
                  session.id === activeSessionId ? " sidebar__row--active" : ""
                }`}
              >
                <button
                  type="button"
                  className="sidebar__item"
                  onClick={() => void openSession(session.id)}
                  title={session.title}
                >
                  {session.title}
                </button>
                <button
                  type="button"
                  className="sidebar__del"
                  aria-label="删除会话"
                  onClick={() => void remove(session.id)}
                >
                  ×
                </button>
              </div>
            ))}
          </section>
        ))}
      </nav>

      <footer className="sidebar__foot">
        {settingsOpen && <SettingsPanel onClose={() => setSettingsOpen(false)} />}
        <button
          type="button"
          className="sidebar__account"
          aria-expanded={settingsOpen}
          aria-label="本地用户"
          onClick={() => setSettingsOpen((open) => !open)}
        >
          <span className="sidebar__avatar" aria-hidden="true">
            本
          </span>
          <span className="sidebar__account-name">本地用户</span>
        </button>
        <button
          type="button"
          className="sidebar__settings"
          aria-expanded={settingsOpen}
          onClick={() => setSettingsOpen((open) => !open)}
        >
          设置
        </button>
      </footer>
    </aside>
  );
}

// 设置面板：模型名、工作区从哪来、密钥写在哪个文件。不能在这里改，改了也要重启
function SettingsPanel({ onClose }: { onClose: () => void }) {
  return (
    <div className="settings" role="dialog" aria-label="设置">
      <div className="settings__title">设置</div>
      <p className="settings__row">
        <span>用户</span>
        本地用户，没有登录
      </p>
      <p className="settings__row">
        <span>模型</span>
        DeepSeek · deepseek-chat
      </p>
      <p className="settings__hint">
        工作区是文件工具和 bash 的根目录。用环境变量 MINI_AGENT_WORKSPACE
        指定；不设的话，就是应用启动时的当前目录。
      </p>
      <p className="settings__hint">
        密钥写在项目根目录的 .env 里：DEEPSEEK_API_KEY=你的密钥。改完需要重启应用。
      </p>
      <button type="button" className="settings__close" onClick={onClose}>
        关闭
      </button>
    </div>
  );
}
