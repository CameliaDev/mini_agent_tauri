// 空态：标题 + 引导语 + 几个示例卡片（点一下填进输入框）
const EXAMPLES = [
  { title: "查时间", text: "现在几点？请用工具查" },
  { title: "看工作区", text: "看看工作区里有哪些文件，用 list_dir" },
];

export function EmptyState({ onPick }: { onPick: (text: string) => void }) {
  return (
    <div className="empty">
      <h1 className="empty__title">今天想聊点什么？</h1>
      <p className="empty__subtitle">
        先在项目根目录的 .env 里写上 DEEPSEEK_API_KEY，然后重启应用。
        点下面的例子会填进输入框，它们都会用到工具。
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
