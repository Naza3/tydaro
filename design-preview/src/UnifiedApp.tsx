import { useCallback, useEffect, useRef, useState } from "react";
import {
  Activity,
  ArrowRight,
  Bell,
  BookOpen,
  Check,
  ChevronRight,
  CircleHelp,
  Compass,
  FileText,
  Layers3,
  LayoutDashboard,
  ListPlus,
  Maximize2,
  Search,
  Settings2,
  ShieldCheck,
  Sparkles,
  TrendingUp,
  X,
} from "lucide-react";
import MarketWorkspace from "./App";
import Chart from "./Chart";
import WatchPage from "./WatchPage";
import { LimitPage, ThemePage } from "./ResearchPages";
import JournalPage from "./JournalPage";
import { stocks, signed, direction, newId, previousClose } from "./data";
import { Dialog } from "./ui";
import {
  DEMO_DATE,
  initialWatchGroups,
  pageNames,
  type DraftRequest,
  type PageActions,
  type PageId,
  type SourceContext,
  type WatchGroup,
} from "./workspace-model";
import "./unified.css";

const navigation = [
  { id: "market", Icon: LayoutDashboard, hint: "行情与自定义看板" },
  { id: "watch", Icon: Activity, hint: "股票分组与异动观察" },
  { id: "limit", Icon: TrendingUp, hint: "涨停资料与多日对照" },
  { id: "theme", Icon: Layers3, hint: "题材周期与证据研究" },
  { id: "journal", Icon: BookOpen, hint: "交易预案与复盘记录" },
] as const;
const pageDescriptions: Record<PageId, string> = {
  market: "观察市场，组织你的工作空间",
  watch: "让值得关注的变化浮到眼前",
  limit: "从每日资料中寻找持续的线索",
  theme: "用证据理解题材的演变",
  journal: "记录判断，为下一次决策做好准备",
};

export default function UnifiedApp() {
  const [page, setPage] = useState<PageId>("market");
  const [groups, setGroups] = useState<WatchGroup[]>(initialWatchGroups);
  const [monitoring, setMonitoring] = useState(true);
  const [muted, setMuted] = useState(true);
  const [alertCount, setAlertCount] = useState(0);
  const [search, setSearch] = useState(false);
  const [query, setQuery] = useState("");
  const [help, setHelp] = useState(false);
  const [preferences, setPreferences] = useState(false);
  const [comfortable, setComfortable] = useState(true);
  const [inspecting, setInspecting] = useState<string | null>(null);
  const [inspectionContext, setInspectionContext] =
    useState<SourceContext | null>(null);
  const [inspectMode, setInspectMode] = useState<"intraday" | "daily">(
    "intraday",
  );
  const [stockRequest, setStockRequest] = useState<{
    code: string;
    sequence: number;
  } | null>(null);
  const [watchRequest, setWatchRequest] = useState<SourceContext | null>(null);
  const [watchTarget, setWatchTarget] = useState(initialWatchGroups[0].id);
  const [draftSource, setDraftSource] = useState<SourceContext | null>(null);
  const [draftDate, setDraftDate] = useState(DEMO_DATE);
  const [journalRequest, setJournalRequest] = useState<DraftRequest | null>(
    null,
  );
  const [themeRequest, setThemeRequest] = useState<DraftRequest | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [journalStatus, setJournalStatus] = useState("复盘保存在当前浏览器");
  const searchTrigger = useRef<HTMLButtonElement>(null);
  const notify = useCallback((message: string) => setNotice(message), []);
  const openSearch = useCallback(() => {
    setQuery("");
    setSearch(true);
  }, []);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.isComposing || event.keyCode === 229) return;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        const anotherDialog = Array.from(
          document.querySelectorAll<HTMLElement>(
            '[role="dialog"][aria-modal="true"]',
          ),
        ).some(
          (dialog) =>
            dialog.getClientRects().length > 0 &&
            !dialog.classList.contains("search-dialog"),
        );
        if (anotherDialog) {
          notify("请先完成或关闭当前弹窗，再使用全局搜索");
          return;
        }
        openSearch();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [openSearch, notify]);
  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 5500);
    return () => window.clearTimeout(timer);
  }, [notice]);
  const openMarket = (code: string) => {
    setStockRequest((old) => ({ code, sequence: (old?.sequence ?? 0) + 1 }));
    setInspecting(null);
    setSearch(false);
    setPage("market");
  };
  const inspect = (code: string, context?: SourceContext) => {
    setInspectionContext(context ?? stockContext(code));
    setInspectMode("intraday");
    setInspecting(code);
  };
  const addToWatch = (context: SourceContext) => {
    setWatchTarget(
      groups.some((group) => group.id === watchTarget)
        ? watchTarget
        : (groups[0]?.id ?? ""),
    );
    setWatchRequest(context);
  };
  const createDraft = (context: SourceContext) => {
    setDraftDate(context.date);
    setDraftSource(context);
  };
  const actions: PageActions = {
    onInspect: inspect,
    onOpenMarket: openMarket,
    onAddToWatch: addToWatch,
    onCreateDraft: createDraft,
    onNotice: notify,
  };
  const selectedStock = stocks.find((stock) => stock.code === inspecting);
  const targetGroup = groups.find((group) => group.id === watchTarget);
  const requestCodes = [...new Set(watchRequest?.codes ?? [])];
  const knownCodes = requestCodes.filter((code) =>
    stocks.some((stock) => stock.code === code),
  );
  const unknownCodes = requestCodes.filter(
    (code) => !knownCodes.includes(code),
  );
  const additions = knownCodes.filter(
    (code) => !targetGroup?.codes.includes(code),
  );
  const duplicateCount = knownCodes.length - additions.length;
  const searchStocks = stocks.filter(
    (stock) =>
      !query ||
      `${stock.name} ${stock.code} ${stock.sector}`
        .toLowerCase()
        .includes(query.toLowerCase().trim()),
  );
  const searchPages = navigation.filter(
    ({ id }) =>
      !query ||
      `${pageNames[id]} ${pageDescriptions[id]}`.includes(query.trim()),
  );
  const stockContext = (code: string): SourceContext => ({
    date: DEMO_DATE,
    sector: stocks.find((s) => s.code === code)?.sector ?? "个股观察",
    codes: [code],
    origin: page,
    reason: "来自个股速览的观察线索",
  });

  return (
    <div className={`unified-app ${comfortable ? "" : "compact-density"}`}>
      <aside className="unified-rail" aria-label="主导航">
        <button
          className="brand-symbol"
          onClick={() => setPage("market")}
          aria-label="Tydaro 首页"
          title="Tydaro"
        >
          <i />
          <i />
          <i />
        </button>
        <div className="rail-section-label">工作台</div>
        <nav>
          {navigation.map(({ id, Icon, hint }) => (
            <button
              key={id}
              data-testid={`nav-${id}`}
              title={hint}
              className={`unified-nav-item ${page === id ? "active" : ""}`}
              aria-current={page === id ? "page" : undefined}
              onClick={() => setPage(id)}
            >
              <Icon size={21} strokeWidth={1.6} />
              <span>{pageNames[id]}</span>
              {id === "watch" && alertCount > 0 && (
                <i className="nav-count">{Math.min(alertCount, 99)}</i>
              )}
            </button>
          ))}
        </nav>
        <div className="rail-bottom">
          <button
            className="unified-nav-item"
            aria-label="显示偏好"
            title="显示偏好"
            onClick={() => setPreferences(true)}
          >
            <Settings2 size={20} />
            <span>偏好</span>
          </button>
          <button
            className="profile-avatar"
            title="本地预览工作空间"
            onClick={() => setHelp(true)}
          >
            T
          </button>
        </div>
      </aside>
      <div className="unified-main">
        <header className="unified-header">
          <div className="unified-heading">
            <span className="unified-wordmark">
              tydaro<span>.</span>
            </span>
            <i />
            <strong>{pageNames[page]}</strong>
            <span className="unified-page-description">
              {pageDescriptions[page]}
            </span>
          </div>
          <div className="unified-header-actions">
            <button
              ref={searchTrigger}
              className="search-trigger unified-search"
              onClick={openSearch}
            >
              <Search size={15} />
              <span>搜索股票、代码或页面</span>
              <kbd>Ctrl K</kbd>
            </button>
            <span className="preview-label">
              <i />
              交互预览
            </span>
            <button
              className={`u-icon-button notification-button ${alertCount ? "has-alert" : ""}`}
              aria-label={`查看异动，${alertCount} 条未读`}
              title="查看异动"
              onClick={() => setPage("watch")}
            >
              <Bell size={18} />
              {alertCount > 0 && <i />}
            </button>
            <button
              className="u-icon-button"
              aria-label="预览使用说明"
              title="使用说明"
              onClick={() => setHelp(true)}
            >
              <CircleHelp size={18} />
            </button>
          </div>
        </header>
        <div className="unified-pages">
          <div
            hidden={page !== "market"}
            className="market-host page-host"
            data-testid="market-page"
          >
            <MarketWorkspace
              embedded
              active={page === "market"}
              stockRequest={stockRequest}
            />
          </div>
          <div hidden={page !== "watch"} className="page-host">
            <WatchPage
              {...actions}
              groups={groups}
              onGroupsChange={setGroups}
              monitoring={monitoring}
              onMonitoringChange={setMonitoring}
              muted={muted}
              onMutedChange={setMuted}
              onAlertCount={setAlertCount}
            />
          </div>
          <div hidden={page !== "limit"} className="page-host">
            <LimitPage
              {...actions}
              onOpenTheme={(context) => {
                setThemeRequest({ ...context, requestId: newId() });
                setPage("theme");
              }}
            />
          </div>
          <div hidden={page !== "theme"} className="page-host">
            <ThemePage {...actions} incoming={themeRequest} />
          </div>
          <div hidden={page !== "journal"} className="page-host">
            <JournalPage
              incoming={journalRequest}
              onInspect={inspect}
              onAddToWatch={addToWatch}
              onNotice={notify}
              onSaveStatus={setJournalStatus}
            />
          </div>
        </div>
        <footer className="unified-status">
          <span>
            <i className={monitoring ? "status-dot" : "status-dot paused"} />
            {monitoring ? "演示监控已启用" : "演示监控已暂停"}
            <span className="footer-divider" />A 股 · 模拟数据 · {DEMO_DATE}
          </span>
          <span>
            <ShieldCheck size={13} />
            {journalStatus}
            <span className="footer-divider" />
            <button onClick={() => setHelp(true)}>
              快捷键 <kbd>Ctrl K</kbd>
            </button>
          </span>
        </footer>
      </div>

      {search && (
        <Dialog
          title="全局搜索"
          subtitle="直达股票或工作页面 · Enter 打开首个匹配结果"
          className="search-dialog"
          onClose={() => {
            setSearch(false);
            searchTrigger.current?.focus();
          }}
        >
          <label className="global-search-input">
            <Search size={20} />
            <input
              data-autofocus
              placeholder="搜索股票、代码或页面"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.nativeEvent.isComposing || event.keyCode === 229)
                  return;
                if (event.key === "Enter") {
                  event.preventDefault();
                  event.stopPropagation();
                  if (searchStocks[0]) openMarket(searchStocks[0].code);
                  else if (searchPages[0]) {
                    setPage(searchPages[0].id);
                    setSearch(false);
                  }
                }
              }}
            />
            <kbd>Esc</kbd>
          </label>
          <div className="global-search-results">
            {searchPages.length > 0 && (
              <>
                <div className="search-section-label">工作页面</div>
                <div className="search-page-links">
                  {searchPages.map(({ id, Icon }) => (
                    <button
                      key={id}
                      onClick={() => {
                        setPage(id);
                        setSearch(false);
                      }}
                    >
                      <Icon size={17} />
                      {pageNames[id]}
                      <ArrowRight size={14} />
                    </button>
                  ))}
                </div>
              </>
            )}
            {searchStocks.length > 0 && (
              <div className="search-section-label">
                股票 <span>{searchStocks.length}</span>
              </div>
            )}
            {searchStocks.map((stock) => (
              <button
                className="global-stock-result"
                key={stock.code}
                onClick={() => openMarket(stock.code)}
              >
                <span className="stock-market-tag">
                  {stock.market === "SH" ? "沪" : "深"}
                </span>
                <span>
                  <strong>{stock.name}</strong>
                  <small>
                    {stock.code} · {stock.sector}
                  </small>
                </span>
                <span
                  className={`search-stock-quote ${direction(stock.change)}`}
                >
                  <b>{stock.price.toFixed(2)}</b>
                  <small>{signed(stock.change)}%</small>
                </span>
                <ChevronRight size={16} />
              </button>
            ))}
            {!searchStocks.length && !searchPages.length && (
              <div className="u-empty">
                <Search size={28} />
                <strong>没有找到匹配结果</strong>
                <span>试试六位股票代码或「题材」「复盘」</span>
              </div>
            )}
          </div>
          <div className="dialog-footnote">
            当前包含 12 只示例股票。研究记录与分组名称不会自动互相覆盖。
          </div>
        </Dialog>
      )}

      {selectedStock && (
        <Dialog
          title="个股速览"
          subtitle="查看个股后，可以继续原来的研究"
          className="stock-inspector"
          onClose={() => setInspecting(null)}
        >
          {inspectionContext && (
            <div className="inspector-source">
              <span className="u-tag teal">
                来自{pageNames[inspectionContext.origin]}
              </span>
              <span>{inspectionContext.sector}</span>
              <time>资料 {inspectionContext.date}</time>
            </div>
          )}
          <div className="inspector-quote">
            <div>
              <span className="u-eyebrow">
                {selectedStock.market} {selectedStock.code}{" "}
                <span>· {selectedStock.sector}</span>
              </span>
              <h2>{selectedStock.name}</h2>
            </div>
            <div
              className={`inspector-price ${direction(selectedStock.change)}`}
            >
              <strong>{selectedStock.price.toFixed(2)}</strong>
              <span>
                {signed(selectedStock.price - previousClose(selectedStock))}　
                {signed(selectedStock.change)}%
              </span>
            </div>
          </div>
          <div className="inspector-chart-tabs">
            <div className="u-segmented">
              {(["intraday", "daily"] as const).map((mode) => (
                <button
                  className={inspectMode === mode ? "active" : ""}
                  onClick={() => setInspectMode(mode)}
                  key={mode}
                >
                  {mode === "intraday" ? "分时" : "日线"}
                </button>
              ))}
            </div>
            <span className="u-muted">10/14 14:32 · 模拟</span>
          </div>
          <div className="inspector-chart">
            <Chart symbol={selectedStock} mode={inspectMode} />
          </div>
          <div className="inspector-context">
            <span>
              <Compass size={15} />
              保留当前页面与筛选条件
            </span>
            <span>单击查看十字线，再次单击收起</span>
          </div>
          <div className="u-dialog-actions">
            <button
              className="u-button"
              onClick={() => {
                const code = selectedStock.code;
                setInspecting(null);
                createDraft(inspectionContext ?? stockContext(code));
              }}
            >
              <FileText size={15} />
              引用到复盘
            </button>
            <button
              className="u-button"
              onClick={() => {
                const code = selectedStock.code;
                setInspecting(null);
                addToWatch(inspectionContext ?? stockContext(code));
              }}
            >
              <ListPlus size={16} />
              加入盯盘
            </button>
            <button
              className="u-button primary"
              onClick={() => openMarket(selectedStock.code)}
            >
              <Maximize2 size={15} />
              打开行情
            </button>
          </div>
        </Dialog>
      )}

      {watchRequest && (
        <Dialog
          title="加入盯盘分组"
          subtitle="股票会追加到目标分组，重复代码自动跳过"
          className="handoff-dialog"
          onClose={() => setWatchRequest(null)}
        >
          <div className="source-summary">
            <span className="u-tag teal">
              来自{pageNames[watchRequest.origin]}
            </span>
            <strong>{watchRequest.sector}</strong>
            <time>{watchRequest.date}</time>
            <p>{watchRequest.reason || "将选中的股票加入独立的观察分组。"}</p>
          </div>
          <label className="u-field">
            <span>目标分组</span>
            <select
              aria-label="目标盯盘分组"
              value={watchTarget}
              onChange={(event) => setWatchTarget(event.target.value)}
            >
              <option value="" disabled>
                请选择分组
              </option>
              {groups.map((group) => (
                <option key={group.id} value={group.id}>
                  {group.name} · {group.codes.length} 只
                </option>
              ))}
            </select>
          </label>
          {!groups.length && (
            <div className="u-empty">
              <p>还没有盯盘分组</p>
              <button
                className="u-button"
                onClick={() => {
                  setWatchRequest(null);
                  setPage("watch");
                }}
              >
                前往创建分组
              </button>
            </div>
          )}
          <div className="handoff-stocks">
            {knownCodes.map((code) => {
              const stock = stocks.find((s) => s.code === code)!;
              const exists = targetGroup?.codes.includes(code);
              return (
                <div key={code}>
                  <span>
                    <b>{stock.name}</b>
                    <small>{code}</small>
                  </span>
                  <span className={exists ? "u-muted" : "handoff-new"}>
                    {exists ? "已存在 · 跳过" : "+ 新增"}
                  </span>
                </div>
              );
            })}
          </div>
          {unknownCodes.length > 0 && (
            <div className="handoff-warning">
              {unknownCodes.join("、")} 不在示例股票目录中，本次跳过。
            </div>
          )}
          <div className="handoff-total">
            新增 <b>{additions.length}</b> 只
            <span>跳过 {duplicateCount} 只重复股票</span>
          </div>
          <div className="u-dialog-actions">
            <button className="u-button" onClick={() => setWatchRequest(null)}>
              取消
            </button>
            <button
              className="u-button primary"
              disabled={!targetGroup || !additions.length}
              onClick={() => {
                setGroups((current) =>
                  current.map((group) =>
                    group.id === watchTarget
                      ? {
                          ...group,
                          codes: [...new Set([...group.codes, ...knownCodes])],
                        }
                      : group,
                  ),
                );
                notify(
                  `已向「${targetGroup?.name}」加入 ${additions.length} 只股票`,
                );
                setWatchRequest(null);
                setPage("watch");
              }}
            >
              <Check size={15} />
              确认加入
            </button>
          </div>
        </Dialog>
      )}

      {draftSource && (
        <Dialog
          title="引用为复盘草稿"
          subtitle="新建一份独立草稿，保留已有预案与其他草稿"
          className="handoff-dialog"
          onClose={() => setDraftSource(null)}
        >
          <div className="source-summary">
            <span className="u-tag teal">
              来自{pageNames[draftSource.origin]}
            </span>
            <strong>{draftSource.sector}</strong>
            <time>资料日期 {draftSource.date}</time>
            <p>
              {draftSource.reason || "保留研究来源，在复盘中补充你的交易判断。"}
            </p>
          </div>
          <label className="u-field">
            <span>草稿业务日期</span>
            <input
              type="date"
              aria-label="草稿业务日期"
              value={draftDate}
              onChange={(event) => setDraftDate(event.target.value)}
            />
          </label>
          <div className="draft-stock-chips">
            {draftSource.codes.map((code) => (
              <span key={code}>
                {stocks.find((stock) => stock.code === code)?.name ?? code}
                <small>{code}</small>
              </span>
            ))}
          </div>
          <div className="handoff-explanation">
            <FileText size={19} />
            <p>
              仅带入题材、股票和来源线索。买卖方法与触发条件由你填写，未完成内容保留为草稿。
            </p>
          </div>
          <div className="u-dialog-actions">
            <button className="u-button" onClick={() => setDraftSource(null)}>
              取消
            </button>
            <button
              className="u-button primary"
              disabled={!/^\d{4}-\d{2}-\d{2}$/.test(draftDate)}
              onClick={() => {
                setJournalRequest({
                  ...draftSource,
                  date: draftDate,
                  reason: `[资料 ${draftSource.date}] ${draftSource.reason ?? "研究线索"}`,
                  requestId: newId(),
                });
                setDraftSource(null);
                setPage("journal");
                notify("已创建独立草稿，请补充你的预案");
              }}
            >
              <FileText size={15} />
              创建草稿
            </button>
          </div>
        </Dialog>
      )}

      {preferences && (
        <Dialog
          title="显示偏好"
          subtitle="让工作区适合你的阅读习惯"
          className="preferences-dialog"
          onClose={() => setPreferences(false)}
        >
          <div className="preference-row">
            <div>
              <strong>内容密度</strong>
              <p>统一调整新工作页面的表格间距</p>
            </div>
            <div className="u-segmented">
              <button
                className={comfortable ? "active" : ""}
                onClick={() => setComfortable(true)}
              >
                舒适
              </button>
              <button
                className={!comfortable ? "active" : ""}
                onClick={() => setComfortable(false)}
              >
                紧凑
              </button>
            </div>
          </div>
          <div className="preference-row">
            <div>
              <strong>浅色 · 青绿</strong>
              <p>延续 Tydaro 当前配色，涨红跌绿</p>
            </div>
            <div className="theme-swatches">
              <i />
              <i />
              <i />
            </div>
          </div>
          <div className="dialog-footnote">
            显示偏好与看板布局在本次预览会话中保留。复盘内容单独保存在当前浏览器。
          </div>
        </Dialog>
      )}

      {help && (
        <Dialog
          title="一个连贯的研究工作台"
          subtitle="Tydaro · 可交互设计预览"
          className="help-dialog"
          onClose={() => setHelp(false)}
        >
          <div className="help-intro">
            <Sparkles size={22} />
            <p>从发现线索到形成预案，保留每一步的上下文。</p>
          </div>
          <div className="help-workflow">
            {[
              {
                page: "limit" as PageId,
                title: "查资料",
                text: "跨日对照、AND 条件筛选",
              },
              {
                page: "theme" as PageId,
                title: "研究题材",
                text: "周期、证据与关联股票",
              },
              {
                page: "journal" as PageId,
                title: "写预案",
                text: "来源引用、草稿与历史记录",
              },
              {
                page: "watch" as PageId,
                title: "加入盯盘",
                text: "独立分组与演示异动",
              },
            ].map((step, index) => (
              <button
                key={step.page}
                onClick={() => {
                  setPage(step.page);
                  setHelp(false);
                }}
              >
                <span>{String(index + 1).padStart(2, "0")}</span>
                <div>
                  <strong>{step.title}</strong>
                  <small>{step.text}</small>
                </div>
                <ArrowRight size={16} />
              </button>
            ))}
          </div>
          <div className="help-notes">
            <p>
              <kbd>Ctrl K</kbd> 搜索股票与页面　<kbd>Esc</kbd> 收起当前弹层
            </p>
            <p>
              行情：单击图表开关十字线；日线双击查看历史分时。添加组件先打开浮窗，再插入看板。
            </p>
            <p>
              所有行情和研究数据均为示例。复盘内容自动保存在当前浏览器；分组、规则与布局仅保留在本次会话。浏览器浮窗用于演示原生多窗口交互。
            </p>
          </div>
        </Dialog>
      )}
      {notice && (
        <div className="unified-toast" role="status">
          <Check size={16} />
          <span>{notice}</span>
          <button aria-label="关闭提示" onClick={() => setNotice(null)}>
            <X size={15} />
          </button>
        </div>
      )}
    </div>
  );
}
