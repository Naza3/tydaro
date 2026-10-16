import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  Bell,
  BellOff,
  Check,
  CheckCheck,
  ChevronRight,
  Ellipsis,
  Folder,
  LayoutGrid,
  ListFilter,
  Pause,
  Play,
  Plus,
  Radio,
  Search,
  Settings2,
  Trash2,
  Volume2,
  VolumeX,
  X,
} from "lucide-react";
import { direction, newId, signed, stocks, type Stock } from "./data";
import {
  DEMO_DATE,
  type PageActions,
  type WatchGroup,
} from "./workspace-model";
import "./WatchPage.css";

type Props = PageActions & {
  groups: WatchGroup[];
  onGroupsChange: (groups: WatchGroup[]) => void;
  monitoring: boolean;
  onMonitoringChange: (value: boolean) => void;
  muted: boolean;
  onMutedChange: (value: boolean) => void;
  onAlertCount?: (count: number) => void;
};
type Rule = {
  id: "rise" | "volume" | "near-limit";
  title: string;
  label: string;
  unit: string;
  threshold: number;
  enabled: boolean;
};
type Alert = {
  id: string;
  code: string;
  ruleId: Rule["id"];
  title: string;
  detail: string;
  groupName: string;
  time: string;
  read: boolean;
};
const initialRules: Rule[] = [
  {
    id: "rise",
    title: "短线拉升",
    label: "3 分钟涨幅达到",
    unit: "%",
    threshold: 1.5,
    enabled: true,
  },
  {
    id: "volume",
    title: "成交放量",
    label: "分钟成交量 / 前 5 分钟均量达到",
    unit: "倍",
    threshold: 2,
    enabled: true,
  },
  {
    id: "near-limit",
    title: "接近涨停",
    label: "距涨停价格不超过",
    unit: "%",
    threshold: 1,
    enabled: true,
  },
];
const hotspotSamples: Record<string, number> = {
  "603598": 8,
  "001318": 4,
  "300750": 3,
  "600519": 2,
  "601127": 7,
  "002594": 5,
  "300059": 6,
  "688981": 9,
  "000333": 2,
  "002475": 7,
  "600036": 2,
  "000858": 3,
};
const initialAlerts: Alert[] = [
  {
    id: "initial-3",
    code: "603598",
    ruleId: "near-limit",
    title: "接近涨停",
    detail: "距涨停 0.01% ≤ 1.00% · 演示值",
    groupName: "传媒 · 情绪观察",
    time: "14:31:42",
    read: false,
  },
  {
    id: "initial-2",
    code: "601127",
    ruleId: "volume",
    title: "成交放量",
    detail: "分钟量比 2.36 倍 ≥ 2.00 倍 · 演示值",
    groupName: "汽车 · 轮动机会",
    time: "14:30:18",
    read: false,
  },
  {
    id: "initial-1",
    code: "001318",
    ruleId: "rise",
    title: "短线拉升",
    detail: "3 分钟涨幅 +1.82% ≥ 1.50% · 演示值",
    groupName: "传媒 · 情绪观察",
    time: "14:28:06",
    read: true,
  },
];

function MiniLine({ stock }: { stock: Stock }) {
  const phase = Number(stock.code.slice(-2));
  const points = Array.from({ length: 25 }, (_, index) => {
    const trend = stock.change >= 0 ? 22 - index * 0.65 : 9 + index * 0.48;
    return `${index * 3},${Math.max(3, Math.min(29, trend + Math.sin(index * 1.65 + phase) * 2.7 + Math.sin(index * 0.57) * 2))}`;
  }).join(" ");
  return (
    <svg
      className={`watch-mini-line ${direction(stock.change)}`}
      viewBox="0 0 72 32"
      aria-hidden="true"
    >
      <path d="M0 25H72" stroke="currentColor" strokeOpacity=".12" />
      <polyline
        points={points}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.35"
      />
    </svg>
  );
}

function WatchDialog({
  title,
  description,
  onClose,
  children,
  drawer = false,
}: {
  title: string;
  description: string;
  onClose: () => void;
  children: ReactNode;
  drawer?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    (
      ref.current?.querySelector<HTMLElement>("[data-autofocus]") ??
      ref.current?.querySelector<HTMLElement>("input, button")
    )?.focus();
    return () => {
      if (previous?.isConnected) previous.focus();
    };
  }, []);
  return (
    <div
      className={`watch-dialog-backdrop ${drawer ? "watch-drawer-backdrop" : ""}`}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={`watch-dialog ${drawer ? "watch-rules-drawer" : ""}`}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing) return;
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            close.current();
          }
          if (event.key === "Tab") {
            const all = Array.from(
              ref.current?.querySelectorAll<HTMLElement>(
                'button:not(:disabled), input:not(:disabled), select, textarea, [tabindex="0"]',
              ) ?? [],
            );
            const first = all[0],
              last = all[all.length - 1];
            if (event.shiftKey && document.activeElement === first) {
              event.preventDefault();
              last?.focus();
            }
            if (!event.shiftKey && document.activeElement === last) {
              event.preventDefault();
              first?.focus();
            }
          }
        }}
      >
        <header className="watch-dialog-heading">
          <div>
            <h2>{title}</h2>
            <p>{description}</p>
          </div>
          <button className="watch-icon" aria-label="关闭" onClick={onClose}>
            <X size={18} />
          </button>
        </header>
        {children}
      </div>
    </div>
  );
}

export default function WatchPage({
  groups,
  onGroupsChange,
  monitoring,
  onMonitoringChange,
  muted,
  onMutedChange,
  onAlertCount,
  onInspect,
  onOpenMarket,
  onCreateDraft,
  onNotice,
}: Props) {
  const [columns, setColumns] = useState(2);
  const [selectedGroup, setSelectedGroup] = useState<string | null>(null);
  const [selectedCode, setSelectedCode] = useState<string | null>(null);
  const [sorting, setSorting] = useState<"stable" | "desc" | "asc">("stable");
  const [alerts, setAlerts] = useState(initialAlerts);
  const [unreadOnly, setUnreadOnly] = useState(false);
  const [rules, setRules] = useState(initialRules);
  const [ruleDraft, setRuleDraft] = useState<Rule[] | null>(null);
  const [editor, setEditor] = useState<WatchGroup | null>(null);
  const [search, setSearch] = useState("");
  const [error, setError] = useState("");
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const demoCounter = useRef(0);
  const unreadCount = alerts.filter((item) => !item.read).length;
  const activeRules = rules.filter((rule) => rule.enabled);
  const uniqueCount = new Set(groups.flatMap((group) => group.codes)).size;
  useEffect(() => onAlertCount?.(unreadCount), [unreadCount, onAlertCount]);
  const goToGroup = (id: string) => {
    setSelectedGroup(id);
    document
      .getElementById(`watch-group-${id}`)
      ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  };
  const editGroup = (group?: WatchGroup) => {
    setEditor(
      group
        ? { ...group, codes: [...group.codes] }
        : { id: newId(), name: "", codes: [], color: "#009b9a", note: "" },
    );
    setSearch("");
    setError("");
    setDeleteConfirm(false);
  };
  const saveGroup = () => {
    if (!editor) return;
    const name = editor.name.trim();
    if (!name || name.length > 24 || /[\u0000-\u001f\u007f]/.test(name)) {
      setError("分组名称为 1–24 个字符，不能包含控制字符。");
      return;
    }
    if (groups.some((group) => group.id !== editor.id && group.name === name)) {
      setError("已有同名分组，请换一个名称。");
      return;
    }
    const saved = { ...editor, name, note: editor.note.trim() };
    onGroupsChange(
      groups.some((group) => group.id === editor.id)
        ? groups.map((group) => (group.id === editor.id ? saved : group))
        : [...groups, saved],
    );
    setEditor(null);
    setSelectedGroup(saved.id);
    onNotice(`已保存「${name}」，共 ${saved.codes.length} 只股票`);
  };
  const inspect = (code: string, groupId?: string) => {
    setSelectedCode(code);
    if (groupId) setSelectedGroup(groupId);
    onInspect(code);
  };
  const addDemoAlert = () => {
    if (!monitoring) return;
    if (!activeRules.length) {
      onNotice("请先启用至少一条预警规则");
      return;
    }
    const available = groups.flatMap((group) =>
      group.codes
        .filter((code) => stocks.some((stock) => stock.code === code))
        .map((code) => ({ code, group })),
    );
    if (!available.length) {
      onNotice("请先向分组添加股票");
      return;
    }
    const count = demoCounter.current++;
    const item = available[count % available.length];
    const rule = activeRules[count % activeRules.length];
    const measured =
      rule.id === "near-limit" ? rule.threshold * 0.48 : rule.threshold + 0.32;
    const detail =
      rule.id === "rise"
        ? `3 分钟涨幅 +${measured.toFixed(2)}% ≥ ${rule.threshold.toFixed(2)}%`
        : rule.id === "volume"
          ? `分钟量比 ${measured.toFixed(2)} 倍 ≥ ${rule.threshold.toFixed(2)} 倍`
          : `距涨停 ${measured.toFixed(2)}% ≤ ${rule.threshold.toFixed(2)}%`;
    setAlerts((previous) =>
      [
        {
          id: newId(),
          code: item.code,
          ruleId: rule.id,
          title: rule.title,
          detail: `${detail} · 演示值 #${count + 1}`,
          groupName: item.group.name,
          time: "14:32:00",
          read: false,
        },
        ...previous,
      ].slice(0, 100),
    );
    onNotice("已生成一条模拟异动，行情价格保持不变");
  };
  const openAlert = (item: Alert) => {
    setAlerts((previous) =>
      previous.map((alert) =>
        alert.id === item.id ? { ...alert, read: true } : alert,
      ),
    );
    const group =
      groups.find(
        (entry) =>
          entry.name === item.groupName && entry.codes.includes(item.code),
      ) ?? groups.find((entry) => entry.codes.includes(item.code));
    if (group) goToGroup(group.id);
    inspect(item.code, group?.id);
  };
  const displayedAlerts = unreadOnly
    ? alerts.filter((alert) => !alert.read)
    : alerts;
  const sortedStocks = (group: WatchGroup) => {
    const members = group.codes
      .map((code) => stocks.find((stock) => stock.code === code))
      .filter((stock): stock is Stock => !!stock);
    return sorting === "stable"
      ? members
      : [...members].sort((a, b) =>
          sorting === "desc" ? b.change - a.change : a.change - b.change,
        );
  };
  const isExisting = editor && groups.some((group) => group.id === editor.id);

  return (
    <div className="u-page watch-page" data-testid="watch-page">
      <header className="u-page-heading watch-page-heading">
        <div>
          <div className="u-eyebrow">
            MARKET MONITOR <span>盘中观察</span>
          </div>
          <div className="watch-heading-title">
            <h1 className="u-page-title">盯盘</h1>
            <span
              className={`watch-monitor-state ${monitoring ? "active" : ""}`}
            >
              <i />
              {monitoring ? "模拟监控中" : "监控已暂停"}
            </span>
          </div>
          <p className="watch-heading-description">
            把关注点放在一起，让异动主动来到你面前。
          </p>
        </div>
        <div className="watch-heading-actions">
          <span className="watch-snapshot">
            {DEMO_DATE} <strong>14:32</strong> 快照
          </span>
          <button
            className="u-button"
            onClick={() => onMutedChange(!muted)}
            aria-pressed={muted}
            title="预览只切换静音状态，不播放音频"
          >
            {muted ? <VolumeX size={15} /> : <Volume2 size={15} />}
            {muted ? "已静音" : "提示音已开"}
          </button>
          <button
            className={`u-button ${monitoring ? "" : "primary"}`}
            onClick={() => onMonitoringChange(!monitoring)}
          >
            {monitoring ? <Pause size={15} /> : <Play size={15} />}
            {monitoring ? "暂停监控" : "继续监控"}
          </button>
        </div>
      </header>

      <div className="watch-workspace">
        <aside className="watch-sidebar u-panel">
          <div className="watch-sidebar-heading">
            <strong>我的分组</strong>
            <button
              className="watch-icon"
              aria-label="新建盯盘分组"
              onClick={() => editGroup()}
            >
              <Plus size={17} />
            </button>
          </div>
          <div className="watch-group-navigation">
            <button
              className={`watch-group-nav ${selectedGroup === null ? "selected" : ""}`}
              onClick={() => setSelectedGroup(null)}
            >
              <LayoutGrid size={16} />
              <span>全部分组</span>
              <b>{groups.length}</b>
            </button>
            {groups.map((group) => (
              <button
                key={group.id}
                className={`watch-group-nav ${selectedGroup === group.id ? "selected" : ""}`}
                onClick={() => goToGroup(group.id)}
              >
                <span
                  className="watch-group-dot"
                  style={{ background: group.color }}
                />
                <span>{group.name}</span>
                <b>{group.codes.length}</b>
              </button>
            ))}
          </div>
          <div className="watch-sidebar-summary">
            <span>关注股票</span>
            <strong>
              {uniqueCount}
              <small>只 · 已去重</small>
            </strong>
          </div>
          <div className="watch-sidebar-rule-heading">
            <strong>
              最新异动 <small>{unreadCount} 条未读</small>
            </strong>
            <button
              className="watch-icon"
              aria-label="设置预警规则"
              onClick={() => setRuleDraft(rules.map((rule) => ({ ...rule })))}
            >
              <Settings2 size={16} />
            </button>
          </div>
          <div className="watch-recent-alerts">
            {alerts.slice(0, 3).map((alert) => (
              <button
                key={alert.id}
                className={`watch-recent-alert ${alert.read ? "" : "unread"}`}
                onClick={() => openAlert(alert)}
                title={alert.detail}
              >
                <span>
                  <i />
                  <strong>
                    {stocks.find((stock) => stock.code === alert.code)?.name ??
                      alert.code}
                  </strong>
                  <time>{alert.time}</time>
                </span>
                <span>
                  <em className={`watch-alert-type ${alert.ruleId}`}>
                    {alert.title}
                  </em>
                  <small>{alert.groupName.split(" · ")[0]}</small>
                </span>
              </button>
            ))}
            {!alerts.length && (
              <div className="watch-recent-empty">
                暂无记录，点击演示异动体验提醒
              </div>
            )}
          </div>
          <button
            className="watch-view-alerts"
            onClick={() =>
              document
                .getElementById("watch-full-alerts")
                ?.scrollIntoView({ behavior: "smooth", block: "nearest" })
            }
          >
            查看全部异动 <ChevronRight size={13} />
          </button>
          <div className="watch-sidebar-bottom">
            <span>
              <i className={monitoring ? "live" : ""} />
              {monitoring ? "规则已就绪" : "已暂停生成异动"}
            </span>
            <button
              aria-label="查看预警规则"
              onClick={() => setRuleDraft(rules.map((rule) => ({ ...rule })))}
            >
              {activeRules.length} 条规则
            </button>
          </div>
        </aside>

        <main className="watch-main">
          <div className="watch-grid-toolbar">
            <div className="watch-toolbar-label">
              <strong>分组看板</strong>
              <span>
                {groups.length} 组 / {uniqueCount} 只股票
              </span>
            </div>
            <div className="watch-toolbar-controls">
              <button
                className={`u-button small ${sorting !== "stable" ? "watch-sorted" : ""}`}
                onClick={() =>
                  setSorting(
                    sorting === "stable"
                      ? "desc"
                      : sorting === "desc"
                        ? "asc"
                        : "stable",
                  )
                }
              >
                {sorting === "desc" ? (
                  <ArrowDown size={14} />
                ) : sorting === "asc" ? (
                  <ArrowUp size={14} />
                ) : (
                  <ArrowUpDown size={14} />
                )}
                {sorting === "stable"
                  ? "手动顺序"
                  : sorting === "desc"
                    ? "涨幅降序"
                    : "涨幅升序"}
              </button>
              <div className="u-segmented watch-columns" aria-label="分组列数">
                {[1, 2, 3].map((number) => (
                  <button
                    key={number}
                    className={columns === number ? "active" : ""}
                    aria-label={`${number} 列布局`}
                    aria-pressed={columns === number}
                    onClick={() => setColumns(number)}
                  >
                    {number} 列
                  </button>
                ))}
              </div>
              <button className="u-button small" onClick={() => editGroup()}>
                <Plus size={14} />
                新建分组
              </button>
            </div>
          </div>

          <div className={`watch-group-grid watch-cols-${columns}`}>
            {groups.map((group) => (
              <section
                key={group.id}
                id={`watch-group-${group.id}`}
                data-testid="watch-group"
                className={`u-panel watch-group-card ${selectedGroup === group.id ? "watch-group-focused" : ""}`}
                style={{ "--watch-group-color": group.color } as CSSProperties}
              >
                <header className="watch-card-header">
                  <div>
                    <span className="watch-color-mark" />
                    <strong>{group.name}</strong>
                    <span className="watch-count">{group.codes.length}</span>
                  </div>
                  <button
                    className="watch-icon"
                    aria-label={`管理分组 ${group.name}`}
                    onClick={() => editGroup(group)}
                  >
                    <Ellipsis size={18} />
                  </button>
                </header>
                <div className="watch-card-note">
                  {group.note || "添加观察备注，记录这个分组的关注点"}
                </div>
                <div
                  className="watch-stock-table"
                  role="table"
                  aria-label={group.name}
                >
                  <div className="watch-stock-header" role="row">
                    <span role="columnheader">名称 / 代码</span>
                    <span className="watch-line-cell" role="columnheader">
                      走势示意
                    </span>
                    <span role="columnheader">最新价</span>
                    <span role="columnheader">涨跌幅</span>
                    <span
                      role="columnheader"
                      title="静态研究样例：演示关联题材与出现次数，不代表已执行历史计算"
                    >
                      月热点 <i>样例</i>
                    </span>
                    <span role="columnheader" aria-label="打开行情" />
                  </div>
                  {sortedStocks(group).map((stock) => (
                    <div
                      key={stock.code}
                      role="row"
                      tabIndex={0}
                      aria-label={`${stock.name} ${stock.code} 查看详情`}
                      className={`watch-stock-row ${selectedGroup === group.id && selectedCode === stock.code ? "selected" : ""}`}
                      onClick={() => inspect(stock.code, group.id)}
                      onKeyDown={(event) => {
                        if (
                          event.target === event.currentTarget &&
                          (event.key === "Enter" || event.key === " ")
                        ) {
                          event.preventDefault();
                          inspect(stock.code, group.id);
                        }
                      }}
                    >
                      <span role="cell" className="watch-stock-name">
                        <strong>{stock.name}</strong>
                        <small>
                          {stock.code} <b>{stock.market}</b>
                        </small>
                      </span>
                      <span role="cell" className="watch-line-cell">
                        <MiniLine stock={stock} />
                      </span>
                      <span role="cell" className={direction(stock.change)}>
                        {stock.price.toFixed(2)}
                      </span>
                      <span
                        role="cell"
                        className={`watch-change ${direction(stock.change)}`}
                      >
                        {signed(stock.change)}%
                      </span>
                      <span role="cell" className="watch-month-hot">
                        {stock.sector}
                        <small>{hotspotSamples[stock.code] ?? 0} 次</small>
                      </span>
                      <button
                        className="watch-row-open"
                        aria-label={`打开 ${stock.name} 行情`}
                        title={`打开 ${stock.name} 行情`}
                        onClick={(event) => {
                          event.stopPropagation();
                          onOpenMarket(stock.code);
                        }}
                      >
                        <ChevronRight size={15} />
                      </button>
                    </div>
                  ))}
                  {!group.codes.length && (
                    <div className="watch-empty-group">
                      <Folder size={22} />
                      <span>这个分组还没有股票</span>
                      <button
                        className="u-button small"
                        onClick={() => editGroup(group)}
                      >
                        添加股票
                      </button>
                    </div>
                  )}
                </div>
                <footer className="watch-card-footer">
                  {selectedGroup === group.id &&
                  selectedCode &&
                  group.codes.includes(selectedCode) ? (
                    <>
                      <span>
                        {
                          stocks.find((stock) => stock.code === selectedCode)
                            ?.name
                        }
                      </span>
                      <div>
                        <button onClick={() => onOpenMarket(selectedCode)}>
                          查看行情 <ChevronRight size={12} />
                        </button>
                        <button
                          onClick={() =>
                            onCreateDraft({
                              date: DEMO_DATE,
                              sector: group.name,
                              codes: [selectedCode],
                              reason: group.note || "来自盯盘分组的观察",
                              origin: "watch",
                            })
                          }
                        >
                          写预案
                        </button>
                      </div>
                    </>
                  ) : (
                    <>
                      <span>单击速览 · 右箭头打开行情</span>
                      <button onClick={() => editGroup(group)}>
                        <Plus size={12} />
                        添加股票
                      </button>
                    </>
                  )}
                </footer>
              </section>
            ))}
            {!groups.length && (
              <div className="u-panel watch-no-groups">
                <Folder size={32} />
                <h3>按自己的观察思路建立分组</h3>
                <p>例如情绪核心、趋势跟踪、轮动候选。</p>
                <button
                  className="u-button primary"
                  onClick={() => editGroup()}
                >
                  <Plus size={15} />
                  创建第一个分组
                </button>
              </div>
            )}
          </div>

          <section id="watch-full-alerts" className="u-panel watch-alert-panel">
            <header className="watch-alert-heading">
              <div>
                <Bell size={16} />
                <strong>异动记录</strong>
                <span className="watch-unread-count">{unreadCount} 条未读</span>
                <span className="watch-alert-caption">
                  最新在上 · 最多保留 100 条
                </span>
              </div>
              <div>
                <button
                  className="u-button small"
                  disabled={!monitoring}
                  onClick={addDemoAlert}
                >
                  <Activity size={14} />
                  演示异动
                </button>
                <button
                  className={`watch-icon ${unreadOnly ? "selected" : ""}`}
                  aria-label={unreadOnly ? "查看全部异动" : "只看未读异动"}
                  aria-pressed={unreadOnly}
                  onClick={() => setUnreadOnly(!unreadOnly)}
                >
                  <ListFilter size={16} />
                </button>
                <button
                  className="watch-icon"
                  aria-label="全部标为已读"
                  disabled={!unreadCount}
                  onClick={() =>
                    setAlerts((previous) =>
                      previous.map((alert) => ({ ...alert, read: true })),
                    )
                  }
                >
                  <CheckCheck size={16} />
                </button>
                <button
                  className="watch-icon"
                  aria-label="清空异动记录"
                  disabled={!alerts.length}
                  onClick={() => setAlerts([])}
                >
                  <Trash2 size={15} />
                </button>
              </div>
            </header>
            <div
              className="watch-alert-table"
              role="table"
              aria-label="异动记录"
            >
              <div className="watch-alert-columns" role="row">
                <span role="columnheader">时间</span>
                <span role="columnheader">股票</span>
                <span role="columnheader">触发规则</span>
                <span role="columnheader">触发依据</span>
                <span role="columnheader">来源分组</span>
              </div>
              <div className="watch-alert-scroll">
                {displayedAlerts.map((alert) => (
                  <button
                    className={`watch-alert-row ${alert.read ? "" : "unread"}`}
                    role="row"
                    data-testid="watch-alert"
                    key={alert.id}
                    onClick={() => openAlert(alert)}
                    aria-label={`${alert.time} ${stocks.find((stock) => stock.code === alert.code)?.name ?? alert.code} ${alert.title}${alert.read ? " 已读" : " 未读"}`}
                  >
                    <span role="cell">
                      <i />
                      {alert.time}
                    </span>
                    <span role="cell">
                      <strong>
                        {stocks.find((stock) => stock.code === alert.code)
                          ?.name ?? alert.code}
                      </strong>
                      <small>{alert.code}</small>
                    </span>
                    <span role="cell">
                      <em className={`watch-alert-type ${alert.ruleId}`}>
                        {alert.title}
                      </em>
                    </span>
                    <span role="cell" title={alert.detail}>
                      {alert.detail}
                    </span>
                    <span role="cell">{alert.groupName}</span>
                  </button>
                ))}
              </div>
              {!displayedAlerts.length && (
                <div className="watch-alert-empty">
                  <BellOff size={21} />
                  <span>
                    {unreadOnly
                      ? "全部读完了，当前没有未读异动"
                      : "暂无异动记录"}
                  </span>
                  <small>
                    {monitoring
                      ? "点击「演示异动」查看完整提醒流程"
                      : "监控已暂停，已有分组和观察状态保持不变"}
                  </small>
                </div>
              )}
            </div>
          </section>
          <p className="watch-bottom-note">
            分组相互独立；排序仅在手动切换时更新。月热点为静态研究样例，静音按钮只预览状态。
          </p>
        </main>
      </div>

      {editor && (
        <WatchDialog
          title={isExisting ? "管理分组" : "新建分组"}
          description="每个分组独立保存名称与股票，移除成员不会影响其他分组。"
          onClose={() => setEditor(null)}
        >
          <div className="watch-editor-fields">
            <label>
              分组名称
              <input
                data-autofocus
                value={editor.name}
                maxLength={24}
                placeholder="例如：传媒 · 情绪观察"
                onChange={(event) => {
                  setEditor({ ...editor, name: event.target.value });
                  setError("");
                }}
              />
            </label>
            <label>
              观察备注<span className="watch-optional">选填</span>
              <input
                value={editor.note}
                maxLength={80}
                placeholder="记录这个分组的关注点"
                onChange={(event) =>
                  setEditor({ ...editor, note: event.target.value })
                }
              />
            </label>
          </div>
          <div className="watch-member-heading">
            <strong>
              分组股票{" "}
              <span>
                {editor.codes.length} / {stocks.length}
              </span>
            </strong>
            <label className="watch-member-search">
              <Search size={14} />
              <input
                aria-label="搜索可添加股票"
                placeholder="名称 / 代码"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
              />
            </label>
          </div>
          <div className="watch-member-list">
            {stocks
              .filter((stock) =>
                `${stock.name}${stock.code}${stock.sector}`.includes(
                  search.trim(),
                ),
              )
              .map((stock) => (
                <label className="watch-member" key={stock.code}>
                  <input
                    type="checkbox"
                    checked={editor.codes.includes(stock.code)}
                    onChange={(event) =>
                      setEditor({
                        ...editor,
                        codes: event.target.checked
                          ? [...editor.codes, stock.code]
                          : editor.codes.filter((code) => code !== stock.code),
                      })
                    }
                  />
                  <span>
                    <strong>{stock.name}</strong>
                    <small>
                      {stock.code} · {stock.sector}
                    </small>
                  </span>
                  <b className={direction(stock.change)}>
                    {signed(stock.change)}%
                  </b>
                </label>
              ))}
            {!stocks.some((stock) =>
              `${stock.name}${stock.code}${stock.sector}`.includes(
                search.trim(),
              ),
            ) && (
              <p className="watch-member-empty">
                未匹配示例股票，请使用已知代码或名称。
              </p>
            )}
          </div>
          <p className="watch-editor-help">
            当前预览目录共 12 只股票；取消勾选即可从本组移除。
          </p>
          {error && (
            <p role="alert" className="watch-form-error">
              {error}
            </p>
          )}
          {deleteConfirm && (
            <div className="watch-delete-confirm">
              <strong>删除「{editor.name}」？</strong>
              <span>只删除这个分组，不影响其他分组中的股票。</span>
              <div>
                <button
                  className="u-button small"
                  onClick={() => setDeleteConfirm(false)}
                >
                  保留分组
                </button>
                <button
                  className="u-button small watch-danger"
                  onClick={() => {
                    onGroupsChange(
                      groups.filter((group) => group.id !== editor.id),
                    );
                    if (selectedGroup === editor.id) setSelectedGroup(null);
                    setEditor(null);
                    onNotice(`已删除「${editor.name}」`);
                  }}
                >
                  确认删除分组
                </button>
              </div>
            </div>
          )}
          <footer className="watch-dialog-footer">
            {isExisting ? (
              <button
                className="watch-delete-button"
                onClick={() => setDeleteConfirm(true)}
              >
                <Trash2 size={14} />
                删除分组
              </button>
            ) : (
              <span />
            )}
            <div>
              <button className="u-button" onClick={() => setEditor(null)}>
                取消
              </button>
              <button className="u-button primary" onClick={saveGroup}>
                <Check size={14} />
                保存分组
              </button>
            </div>
          </footer>
        </WatchDialog>
      )}

      {ruleDraft && (
        <WatchDialog
          drawer
          title="预警规则"
          description="设置哪些变化值得提醒。规则作用于全部盯盘分组。"
          onClose={() => setRuleDraft(null)}
        >
          <div className="watch-rules-body">
            <div className="watch-rules-intro">
              <Radio size={18} />
              <p>
                <strong>模拟规则配置</strong>
                <span>
                  保存后用于手动演示异动；不在后台轮询或执行真实行情策略。
                </span>
              </p>
            </div>
            {ruleDraft.map((rule) => (
              <section
                className={`watch-rule-editor ${rule.enabled ? "" : "disabled"}`}
                key={rule.id}
              >
                <header>
                  <div>
                    <strong>{rule.title}</strong>
                    <span>
                      {rule.id === "rise"
                        ? "发现短时间的价格变化"
                        : rule.id === "volume"
                          ? "观察成交活跃度突然上升"
                          : "关注临近涨停的价格变化"}
                    </span>
                  </div>
                  <button
                    className={`watch-switch ${rule.enabled ? "on" : ""}`}
                    role="switch"
                    aria-checked={rule.enabled}
                    aria-label={`启用${rule.title}`}
                    onClick={() =>
                      setRuleDraft(
                        ruleDraft.map((item) =>
                          item.id === rule.id
                            ? { ...item, enabled: !item.enabled }
                            : item,
                        ),
                      )
                    }
                  >
                    <i />
                  </button>
                </header>
                <label>
                  <span>{rule.label}</span>
                  <div>
                    <input
                      aria-label={`${rule.title}阈值`}
                      type="number"
                      min="0.1"
                      max={rule.id === "volume" ? 100 : 20}
                      step="0.1"
                      value={
                        Number.isFinite(rule.threshold) ? rule.threshold : ""
                      }
                      disabled={!rule.enabled}
                      onChange={(event) =>
                        setRuleDraft(
                          ruleDraft.map((item) =>
                            item.id === rule.id
                              ? {
                                  ...item,
                                  threshold:
                                    event.target.value === ""
                                      ? NaN
                                      : Number(event.target.value),
                                }
                              : item,
                          ),
                        )
                      }
                    />
                    <span>{rule.unit}</span>
                  </div>
                </label>
                <p>
                  判断方式：
                  {rule.id === "near-limit" ? "距离 ≤ 阈值" : "实测值 ≥ 阈值"}
                  ；提醒保留触发时的规则和数值。
                </p>
              </section>
            ))}
            <p className="watch-rules-footnote">
              暂停监控会阻止生成新异动，静音只改变提示音状态；两者都保留已有记录。
            </p>
          </div>
          <footer className="watch-dialog-footer">
            <span>
              {ruleDraft.filter((rule) => rule.enabled).length} 条规则已启用
            </span>
            <div>
              <button className="u-button" onClick={() => setRuleDraft(null)}>
                取消
              </button>
              <button
                className="u-button primary"
                disabled={ruleDraft.some(
                  (rule) =>
                    rule.enabled &&
                    (!Number.isFinite(rule.threshold) ||
                      rule.threshold < 0.1 ||
                      rule.threshold > (rule.id === "volume" ? 100 : 20)),
                )}
                onClick={() => {
                  setRules(ruleDraft);
                  setRuleDraft(null);
                  onNotice("预警规则已保存，用于后续演示异动");
                }}
              >
                保存规则
              </button>
            </div>
          </footer>
        </WatchDialog>
      )}
    </div>
  );
}
