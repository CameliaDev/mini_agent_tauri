// 空态：标题 + 引导语 + 几个示例卡片（点一下填进输入框）
const EXAMPLES = [
  { title: "解释概念", text: "用一句话解释 Rust 的所有权" },
  { title: "调用工具", text: "现在几点？用工具查一下" },
  { title: "写点东西", text: "写一段关于 Tauri Channel 的说明" },
  { title: "头脑风暴", text: "给我三个关于桌面 Agent 的产品点子" },
];

export function EmptyState({ onPick }: { onPick: (text: string) => void }) {
  return (
    <div className="empty">
      <h1 className="empty__title">今天想聊点什么？</h1>
      <p className="empty__subtitle">
        我可以回答问题、调用工具、帮你把想法写成文字。
      </p>
      <div className="empty__grid">
        {EXAMPLES.map((item) => (
          <button
            key={item.title}
            type="button"
            className="empty__card"
            onClick={() => onPick(item.text)}
          >
            <span className="empty__card-title">{item.title}</span>
            <span className="empty__card-text">{item.text}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
