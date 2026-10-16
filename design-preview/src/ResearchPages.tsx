import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowDownLeft,
  ArrowRight,
  ArrowUpRight,
  BookOpen,
  CalendarDays,
  Check,
  CheckCheck,
  ChevronDown,
  ChevronRight,
  CircleHelp,
  Copy,
  Database,
  FilePenLine,
  FileText,
  Filter,
  Link2,
  Plus,
  Search,
  Sparkles,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { newId, stocks } from "./data";
import type {
  DraftRequest,
  PageActions,
  SourceContext,
} from "./workspace-model";
import "./ResearchPages.css";

type RecordRow = {
  id: string;
  date: string;
  sector: string;
  code: string;
  name: string;
  board: string;
  reason: string;
  source: string;
};
type FilterItem = { id: string; field: string; value: string };
const dates = [
  "2026-10-14",
  "2026-10-13",
  "2026-10-12",
  "2026-10-09",
  "2026-10-08",
  "2026-10-07",
];
const sectorOrder = [
  ["AI 应用", "汽车产业链", "消费复苏", "科技硬件"],
  ["汽车产业链", "AI 应用", "科技硬件", "消费复苏"],
  ["消费复苏", "科技硬件", "AI 应用", "汽车产业链"],
  ["AI 应用", "科技硬件", "汽车产业链", "消费复苏"],
  ["科技硬件", "消费复苏", "AI 应用", "汽车产业链"],
  ["汽车产业链", "消费复苏", "科技硬件", "AI 应用"],
];
const sectors: Record<
  string,
  { codes: string[]; reason: string; color: string }
> = {
  "AI 应用": {
    codes: ["603598", "300059", "002475"],
    reason: "内容生产与营销应用受到关注，观察核心标的与板块跟随的持续性。",
    color: "teal",
  },
  汽车产业链: {
    codes: ["601127", "002594", "300750"],
    reason: "新车型发布与产业链需求预期共振，关注整车及电池方向的承接。",
    color: "blue",
  },
  消费复苏: {
    codes: ["001318", "600519", "000858", "000333"],
    reason: "消费刺激预期延续，乳业、白酒及家电呈现轮动，关注量能变化。",
    color: "amber",
  },
  科技硬件: {
    codes: ["688981", "002475", "000333"],
    reason: "国产替代与终端创新预期带动关注，观察半导体与消费电子的联动。",
    color: "purple",
  },
};
const reasons: Record<string, string> = {
  "603598": "AI 营销 + 数字传媒 + 内容生产",
  "300059": "互联网金融 + AI 应用",
  "002475": "消费电子 + AI 终端 + 产业链",
  "601127": "新能源汽车 + 智能驾驶",
  "002594": "新能源汽车 + 海外布局",
  "300750": "动力电池 + 储能",
  "001318": "乳业 + 消费复苏 + 区域品牌",
  "600519": "白酒 + 消费复苏",
  "000858": "白酒 + 消费复苏",
  "000333": "家用电器 + 智能制造",
  "688981": "半导体 + 国产替代",
};
function sampleRows(date = "", dayIndex = 0): RecordRow[] {
  return sectorOrder[dayIndex % sectorOrder.length].flatMap(
    (sector, sectorIndex) => {
      const count = (dayIndex + sectorIndex) % 3 === 1 ? 2 : 3;
      return sectors[sector].codes.slice(0, count).map((code, index) => {
        const stock = stocks.find((s) => s.code === code)!;
        return {
          id: `${date}-${sector}-${code}`,
          date,
          sector,
          code,
          name: stock.name,
          board:
            index === 0
              ? ["4天4板", "3天3板", "2天2板", "首板"][dayIndex % 4]
              : index === 1
                ? "2天2板"
                : "首板",
          reason: reasons[code],
          source: `${date}_涨停复盘_示例.csv`,
        };
      });
    },
  );
}
const seedRows = dates.flatMap((date, index) => sampleRows(date, index));
const fieldLabels: Record<string, string> = {
  all: "全部字段",
  sector: "板块",
  code: "股票代码",
  name: "股票名称",
  board: "连板字段",
  reason: "关键词",
  date: "交易日",
  source: "来源文件",
};
function matches(row: RecordRow, conditions: FilterItem[]) {
  return conditions.every(({ field, value }) =>
    (field === "all"
      ? Object.values(row).join(" ")
      : row[field as keyof RecordRow]
    )
      .toLocaleLowerCase()
      .includes(value.toLocaleLowerCase()),
  );
}
function contextFor(
  rows: RecordRow[],
  origin: "limit" | "theme" = "limit",
): SourceContext {
  return {
    date: rows[0]?.date ?? dates[0],
    sector: rows[0]?.sector ?? "研究观察",
    codes: [...new Set(rows.map((r) => r.code))],
    reason: rows[0]?.reason ?? "",
    origin,
  };
}
function Modal({
  title,
  children,
  close,
  wide = false,
  drawer = false,
}: {
  title: string;
  children: React.ReactNode;
  close: () => void;
  wide?: boolean;
  drawer?: boolean;
}) {
  const panel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement;
    const node = panel.current;
    (
      node?.querySelector<HTMLElement>("input, textarea, select") ??
      node?.querySelector<HTMLElement>("button")
    )?.focus();
    return () => previous?.focus();
  }, []);
  return (
    <div
      className={`research-overlay ${drawer ? "research-drawer-overlay" : ""}`}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) close();
      }}
    >
      <div
        ref={panel}
        className={`research-modal ${wide ? "wide" : ""} ${drawer ? "research-drawer" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing || event.keyCode === 229) return;
          if (event.key === "Escape") {
            event.stopPropagation();
            close();
          }
          if (event.key === "Tab") {
            const elements = Array.from(
              panel.current?.querySelectorAll<HTMLElement>(
                'button:not(:disabled),input,textarea,select,[tabindex="0"]',
              ) ?? [],
            );
            const first = elements[0],
              last = elements[elements.length - 1];
            if (event.shiftKey && document.activeElement === first) {
              event.preventDefault();
              last?.focus();
            } else if (!event.shiftKey && document.activeElement === last) {
              event.preventDefault();
              first?.focus();
            }
          }
        }}
      >
        <div className="research-modal-title">
          <h2>{title}</h2>
          <button
            className="u-button small"
            aria-label={`关闭${title}`}
            onClick={close}
          >
            <X size={16} />
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

export function LimitPage(
  props: PageActions & { onOpenTheme: (context: SourceContext) => void },
) {
  const [rows, setRows] = useState(seedRows);
  const [selectedDate, setSelectedDate] = useState(dates[0]);
  const [selectedSector, setSelectedSector] = useState("AI 应用");
  const [selectedCode, setSelectedCode] = useState("603598");
  const [tab, setTab] = useState<"details" | "search">("details");
  const [matrixCollapsed, setMatrixCollapsed] = useState(false);
  const [conditions, setConditions] = useState<FilterItem[]>([]);
  const [filterField, setFilterField] = useState("all");
  const [filterValue, setFilterValue] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [deleteDate, setDeleteDate] = useState<string | null>(null);
  const [edit, setEdit] = useState<RecordRow | null>(null);
  const [copyFallback, setCopyFallback] = useState<string | null>(null);
  const [deleted, setDeleted] = useState<RecordRow[] | null>(null);
  const actualDates = [...new Set(rows.map((row) => row.date))]
    .sort()
    .reverse();
  const imported = actualDates.includes("2026-10-15");
  const selectedRows = rows.filter(
    (row) => row.date === selectedDate && row.sector === selectedSector,
  );
  const results = useMemo(
    () => rows.filter((row) => matches(row, conditions)),
    [rows, conditions],
  );
  const resultIds = new Set(results.map((row) => row.id));
  const displayed = tab === "search" ? results : selectedRows;
  const selectedRow =
    selectedRows.find((r) => r.code === selectedCode) ?? selectedRows[0];
  const currentContext = contextFor(selectedRows);
  const copiedRows = tab === "search" ? results : selectedRows;
  const selectSector = (date: string, sector: string) => {
    setSelectedDate(date);
    setSelectedSector(sector);
    setSelectedCode(
      rows.find((row) => row.date === date && row.sector === sector)?.code ??
        "",
    );
    setTab("details");
  };
  const addFilter = () => {
    if (!filterValue.trim()) return;
    setConditions((old) => [
      ...old,
      { id: newId(), field: filterField, value: filterValue.trim() },
    ]);
    setFilterValue("");
    setTab("search");
  };
  const copyCodes = async () => {
    const text = [...new Set(copiedRows.map((row) => row.code))].join("\n");
    if (!text) {
      props.onNotice("当前没有可复制的股票代码");
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
      props.onNotice(`已复制 ${text.split("\n").length} 个股票代码`);
    } catch {
      setCopyFallback(text);
    }
  };
  const confirmImport = () => {
    if (imported) return;
    const next = sampleRows("2026-10-15", 0);
    setRows((old) => [...next, ...old]);
    setImportOpen(false);
    setSelectedDate("2026-10-15");
    setSelectedSector("AI 应用");
    setTab("details");
    props.onNotice("已载入 10 月 15 日示例资料，可删除整日撤销");
  };
  return (
    <section
      className="u-page research-page limit-page"
      data-testid="limit-page"
    >
      <header className="u-page-heading">
        <div>
          <div className="u-eyebrow">整理资料 · 发现线索</div>
          <h1 className="u-page-title">
            涨停资料 <span className="research-title-tag">导入资料库</span>
          </h1>
        </div>
        <div className="research-heading-actions">
          <span className="research-local">
            <Database size={13} /> 本地示例 · {actualDates.length} 个交易日
          </span>
          <button
            className="u-button primary"
            onClick={() => setImportOpen(true)}
          >
            <Upload size={15} />
            导入资料
          </button>
        </div>
      </header>
      <div className="research-toolbar limit-searchbar">
        <Filter size={16} className="u-muted" />
        <span className="research-toolbar-label">筛选资料</span>
        <select
          aria-label="搜索字段"
          value={filterField}
          onChange={(event) => setFilterField(event.target.value)}
        >
          {Object.entries(fieldLabels).map(([key, name]) => (
            <option value={key} key={key}>
              {name}
            </option>
          ))}
        </select>
        <div className="research-search-input">
          <Search size={14} />
          <input
            aria-label="涨停搜索条件"
            value={filterValue}
            placeholder={
              filterField === "date"
                ? "2026-10-14"
                : "输入关键词，按 Enter 添加条件"
            }
            onChange={(event) => setFilterValue(event.target.value)}
            onKeyDown={(event) => {
              if (
                event.key === "Enter" &&
                !event.nativeEvent.isComposing &&
                event.keyCode !== 229
              )
                addFilter();
            }}
          />
        </div>
        <button
          className="u-button small"
          onClick={addFilter}
          disabled={!filterValue.trim()}
        >
          <Plus size={14} />
          添加条件
        </button>
        <span className="research-and-hint">多条件同时满足</span>
      </div>
      {conditions.length > 0 && (
        <div className="research-filter-chips">
          <span className="u-muted">AND</span>
          {conditions.map((condition) => (
            <button
              className="research-condition"
              key={condition.id}
              onClick={() =>
                setConditions((old) =>
                  old.filter((item) => item.id !== condition.id),
                )
              }
              aria-label={`移除条件 ${fieldLabels[condition.field]} ${condition.value}`}
            >
              {fieldLabels[condition.field]} <strong>{condition.value}</strong>
              <X size={12} />
            </button>
          ))}
          <button
            className="research-text-button"
            onClick={() => {
              setConditions([]);
              setTab("details");
            }}
          >
            清空条件
          </button>
          <span className="research-result-count">{results.length} 条命中</span>
        </div>
      )}
      <div
        className={`u-panel limit-matrix-panel ${matrixCollapsed ? "collapsed" : ""}`}
      >
        <div className="u-panel-heading">
          <div className="research-inline">
            <CalendarDays size={15} />
            <strong>多日板块概览</strong>
            <span className="u-tag">最近 {actualDates.length} 个导入日</span>
          </div>
          <div className="research-inline">
            <span className="research-note">
              <CircleHelp size={13} /> 每日按原资料顺序排列，列不代表同名板块
            </span>
            <button
              className="research-text-button"
              aria-label={matrixCollapsed ? "展开多日概览" : "收起多日概览"}
              onClick={() => setMatrixCollapsed(!matrixCollapsed)}
            >
              {matrixCollapsed ? "展开" : "收起"}
              <ChevronDown
                size={13}
                style={{
                  transform: matrixCollapsed ? undefined : "rotate(180deg)",
                }}
              />
            </button>
          </div>
        </div>
        <div className="limit-matrix-scroll">
          <div className="limit-matrix-header">
            <span>交易日</span>
            <span>
              板块分布 <span className="u-muted">/ 数字为资料条数</span>
            </span>
            <span>合计</span>
          </div>
          {actualDates.map((date) => {
            const dayRows = rows.filter((row) => row.date === date);
            const names = [...new Set(dayRows.map((row) => row.sector))];
            return (
              <div
                className={`limit-matrix-row ${date === selectedDate ? "is-date-selected" : ""}`}
                key={date}
              >
                <button
                  className="limit-date"
                  onClick={() => selectSector(date, names[0])}
                >
                  <strong>{date.slice(5).replace("-", ".")}</strong>
                  <span>
                    {date === dates[0]
                      ? "周三"
                      : date === "2026-10-15"
                        ? "周四"
                        : [
                            "周日",
                            "周一",
                            "周二",
                            "周三",
                            "周四",
                            "周五",
                            "周六",
                          ][new Date(`${date}T12:00:00`).getDay()]}
                  </span>
                </button>
                <div className="limit-sector-grid">
                  {names.map((sector) => {
                    const items = dayRows.filter(
                      (row) => row.sector === sector,
                    );
                    const match =
                      conditions.length > 0 &&
                      items.some((row) => resultIds.has(row.id));
                    return (
                      <button
                        key={sector}
                        aria-label={`${date} ${sector} ${items.length} 条`}
                        className={`limit-sector ${sectors[sector]?.color ?? "teal"} ${date === selectedDate && selectedSector === sector ? "selected" : ""} ${match ? "has-result" : ""} ${conditions.length > 0 && !match ? "dimmed" : ""}`}
                        onClick={() => selectSector(date, sector)}
                      >
                        <span>{sector}</span>
                        <strong>{items.length}</strong>
                        {match && <i />}
                      </button>
                    );
                  })}
                </div>
                <strong className="limit-day-total">{dayRows.length}</strong>
              </div>
            );
          })}
        </div>
        <div className="research-panel-foot">
          <span>
            <CheckCheck size={13} /> 原始顺序已保留
          </span>
          <span>示例数据用于验证研究流程，不代表实际涨停结果</span>
        </div>
      </div>
      <div className="u-panel limit-details-panel">
        <div className="u-panel-heading limit-details-heading">
          <div className="research-tabs">
            <button
              className={tab === "details" ? "active" : ""}
              onClick={() => setTab("details")}
            >
              板块明细 <span>{selectedRows.length}</span>
            </button>
            <button
              className={tab === "search" ? "active" : ""}
              onClick={() => setTab("search")}
            >
              搜索结果 <span>{results.length}</span>
            </button>
          </div>
          <div className="research-inline">
            <button className="u-button small" onClick={copyCodes}>
              <Copy size={13} />
              复制代码
            </button>
            <button
              className="u-button small"
              disabled={
                !selectedDate || !rows.some((row) => row.date === selectedDate)
              }
              onClick={() => setDeleteDate(selectedDate)}
              aria-label="删除当前整日资料"
            >
              <Trash2 size={13} />
              删除整日
            </button>
          </div>
        </div>
        {tab === "details" ? (
          <div className="limit-sector-summary">
            <div className="limit-sector-heading">
              <div className="research-inline">
                <span
                  className={`research-sector-dot ${sectors[selectedSector]?.color ?? "teal"}`}
                />
                <h2>{selectedSector}</h2>
                <span className="u-tag">{selectedDate}</span>
              </div>
              <div className="research-inline">
                <button
                  className="u-button small"
                  disabled={!selectedRows.length}
                  onClick={() => props.onOpenTheme(currentContext)}
                >
                  <Sparkles size={13} />
                  查看题材
                </button>
                <button
                  className="u-button small"
                  disabled={!selectedRows.length}
                  onClick={() => props.onAddToWatch(currentContext)}
                >
                  <Plus size={13} />
                  加入盯盘
                </button>
                <button
                  className="u-button small"
                  disabled={!selectedRows.length}
                  onClick={() => props.onCreateDraft(currentContext)}
                >
                  <FilePenLine size={13} />
                  引用为预案
                </button>
              </div>
            </div>
            <p>{sectors[selectedSector]?.reason}</p>
          </div>
        ) : (
          <div className="limit-search-summary">
            <Search size={15} />
            <span>
              {conditions.length
                ? `${conditions.length} 个条件同时满足，共 ${results.length} 条资料`
                : "全部已导入资料"}
            </span>
            <span className="u-muted">点击定位回原日期与板块</span>
          </div>
        )}
        <div className="limit-table-scroll">
          <table className="u-data-table limit-table">
            <thead>
              <tr>
                <th>股票 / 代码</th>
                {tab === "search" && <th>日期 · 板块</th>}
                <th>连板文本</th>
                <th>涨停关键词</th>
                <th>资料来源</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {displayed.map((row) => (
                <tr
                  key={row.id}
                  className={`${selectedRow?.id === row.id ? "selected" : ""} ${conditions.length && resultIds.has(row.id) ? "matched" : ""}`}
                >
                  <td>
                    <button
                      className="research-stock-button"
                      onClick={() => {
                        setSelectedCode(row.code);
                        props.onInspect(row.code, {
                          date: row.date,
                          sector: row.sector,
                          codes: [row.code],
                          reason: row.reason,
                          origin: "limit",
                        });
                      }}
                    >
                      <strong>{row.name}</strong>
                      <span>{row.code}</span>
                    </button>
                  </td>
                  {tab === "search" && (
                    <td>
                      <span>{row.date.slice(5)}</span>
                      <span className="research-table-sub">{row.sector}</span>
                    </td>
                  )}
                  <td>
                    <span
                      className={`research-board-tag ${row.board.includes("4") ? "high" : ""}`}
                    >
                      {row.board}
                    </span>
                  </td>
                  <td>
                    <div className="research-keywords">
                      {row.reason.split(" + ").map((reason) => (
                        <span key={reason}>{reason}</span>
                      ))}
                    </div>
                  </td>
                  <td>
                    <span className="research-source">
                      <FileText size={12} />
                      {row.source.replace("_涨停复盘_示例.csv", "_示例.csv")}
                    </span>
                  </td>
                  <td>
                    <div className="research-row-actions">
                      {tab === "search" && (
                        <button
                          aria-label={`定位 ${row.date} ${row.name}`}
                          onClick={() => selectSector(row.date, row.sector)}
                        >
                          定位
                          <ArrowDownLeft size={12} />
                        </button>
                      )}
                      <button
                        aria-label={`编辑 ${row.date} ${row.sector} ${row.name}`}
                        onClick={() => setEdit({ ...row })}
                      >
                        编辑
                        <FilePenLine size={12} />
                      </button>
                      <button
                        aria-label={`查看 ${row.name} 行情`}
                        onClick={() => props.onOpenMarket(row.code)}
                      >
                        <ArrowUpRight size={15} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {!displayed.length && (
            <div className="u-empty">
              <Search size={24} />
              <strong>没有匹配的资料</strong>
              <span>移除筛选条件，或载入示例资料后再试。</span>
            </div>
          )}
        </div>
        <div className="research-panel-foot">
          <span>
            <Database size={13} /> 资料库独立于实时行情
          </span>
          <span>
            {tab === "details"
              ? `${selectedRows.length} 条记录 · 保留原文件行序`
              : `${results.length} 条结果 · AND 交集`}
          </span>
        </div>
      </div>
      {deleted && (
        <div className="research-undo">
          <Check size={14} />
          <span>已移除 {deleted[0]?.date} 的全部示例资料</span>
          <button
            onClick={() => {
              setRows((old) => [
                ...new Map(
                  [...deleted, ...old].map((row) => [row.id, row]),
                ).values(),
              ]);
              setSelectedDate(deleted[0].date);
              setSelectedSector(deleted[0].sector);
              setDeleted(null);
            }}
          >
            撤销
          </button>
          <button aria-label="关闭删除提示" onClick={() => setDeleted(null)}>
            <X size={14} />
          </button>
        </div>
      )}
      {importOpen && (
        <Modal title="导入涨停资料" close={() => setImportOpen(false)}>
          <div className="research-import-hero">
            <div>
              <Upload size={26} />
            </div>
            <h3>先预览，再写入资料库</h3>
            <p>
              这份交互预览提供一份本地示例。正式版本将支持原有 CSV / TXT
              格式与重复文件检测。
            </p>
          </div>
          <div className="research-import-file">
            <FileText size={24} />
            <div>
              <strong>2026-10-15_涨停复盘_示例.csv</strong>
              <span>
                4 个板块 · {sampleRows("2026-10-15").length} 条记录 · 模拟资料
              </span>
            </div>
            <span className={`u-tag ${imported ? "" : "teal"}`}>
              {imported ? "已载入" : "可载入"}
            </span>
          </div>
          <div className="research-import-preview">
            {sectorOrder[0].map((sector) => (
              <span key={sector}>
                {sector}
                <b>
                  {
                    sampleRows("2026-10-15").filter(
                      (row) => row.sector === sector,
                    ).length
                  }
                </b>
              </span>
            ))}
          </div>
          <div className="research-modal-footer">
            <button className="u-button" onClick={() => setImportOpen(false)}>
              取消
            </button>
            <button
              className="u-button primary"
              disabled={imported}
              onClick={confirmImport}
            >
              {imported ? "示例已在资料库中" : "确认载入示例"}
            </button>
          </div>
        </Modal>
      )}
      {deleteDate && (
        <Modal title="删除整日资料" close={() => setDeleteDate(null)}>
          <div className="research-confirm-body">
            <Trash2 size={24} />
            <h3>删除 {deleteDate} 的资料？</h3>
            <p>
              将移除这一天全部板块及{" "}
              {rows.filter((row) => row.date === deleteDate).length}{" "}
              条明细。本预览中可以撤销；其他日期保持不变。
            </p>
          </div>
          <div className="research-modal-footer">
            <button className="u-button" onClick={() => setDeleteDate(null)}>
              保留资料
            </button>
            <button
              className="u-button research-danger"
              onClick={() => {
                const removed = rows.filter((row) => row.date === deleteDate);
                const rest = rows.filter((row) => row.date !== deleteDate);
                setDeleted(removed);
                setRows(rest);
                setSelectedDate(rest[0]?.date ?? "");
                setSelectedSector(rest[0]?.sector ?? "");
                setDeleteDate(null);
              }}
            >
              确认删除整日
            </button>
          </div>
        </Modal>
      )}
      {edit && (
        <Modal title="编辑原始资料" drawer close={() => setEdit(null)}>
          <div className="research-edit-form">
            <p className="research-edit-context">
              {edit.date} · {edit.sector} · {edit.code}
            </p>
            <label>
              股票显示名称
              <input
                value={edit.name}
                onChange={(event) =>
                  setEdit({ ...edit, name: event.target.value })
                }
              />
            </label>
            <label>
              连板文本
              <input
                value={edit.board}
                placeholder="例如 4天4板"
                onChange={(event) =>
                  setEdit({ ...edit, board: event.target.value })
                }
              />
            </label>
            <label>
              涨停关键词
              <textarea
                rows={4}
                value={edit.reason}
                onChange={(event) =>
                  setEdit({ ...edit, reason: event.target.value })
                }
              />
            </label>
            <p className="research-note">
              仅修改此条导入资料，不修改证券目录中的名称。
            </p>
          </div>
          <div className="research-modal-footer">
            <button className="u-button" onClick={() => setEdit(null)}>
              取消
            </button>
            <button
              className="u-button primary"
              disabled={!edit.name.trim() || !edit.board.trim()}
              onClick={() => {
                setRows((old) =>
                  old.map((row) =>
                    row.id === edit.id
                      ? {
                          ...edit,
                          name: edit.name.trim(),
                          board: edit.board.trim(),
                        }
                      : row,
                  ),
                );
                setEdit(null);
                props.onNotice("已更新这条示例资料");
              }}
            >
              保存修改
            </button>
          </div>
        </Modal>
      )}
      {copyFallback && (
        <Modal title="复制股票代码" close={() => setCopyFallback(null)}>
          <div className="research-edit-form">
            <p>浏览器未允许直接写入剪贴板。请选择以下代码并复制。</p>
            <textarea
              aria-label="待复制股票代码"
              rows={8}
              readOnly
              value={copyFallback}
              onFocus={(event) => event.currentTarget.select()}
            />
          </div>
          <div className="research-modal-footer">
            <button
              className="u-button primary"
              onClick={() => setCopyFallback(null)}
            >
              完成
            </button>
          </div>
        </Modal>
      )}
    </section>
  );
}

type ThemeObject = {
  code: string;
  trigger: string;
  theme: string;
  secondary: string;
  start: string;
  method: string;
  keyword: string;
  related: string[];
  generic: string[];
  counts: number[];
  first: number[];
  second: number[];
};
const themeObjects: ThemeObject[] = [
  {
    code: "603598",
    trigger: "2026-10-14",
    theme: "AI 应用",
    secondary: "文化传媒",
    start: "2026-10-09",
    method: "连续识别 + 字段识别",
    keyword: "AI 营销",
    related: ["300059", "002475"],
    generic: ["688981"],
    counts: [4, 6, 8, 9],
    first: [3, 4, 5, 5],
    second: [1, 1, 2, 2],
  },
  {
    code: "001318",
    trigger: "2026-10-13",
    theme: "消费复苏",
    secondary: "食品饮料",
    start: "2026-10-08",
    method: "字段识别",
    keyword: "消费复苏",
    related: ["600519", "000858", "000333"],
    generic: ["600036"],
    counts: [3, 5, 4, 7],
    first: [2, 3, 2, 4],
    second: [1, 1, 1, 2],
  },
  {
    code: "601127",
    trigger: "2026-10-12",
    theme: "汽车产业链",
    secondary: "智能驾驶",
    start: "2026-10-07",
    method: "连续识别",
    keyword: "新能源汽车",
    related: ["002594", "300750"],
    generic: ["002475"],
    counts: [5, 6, 5, 8],
    first: [4, 3, 3, 5],
    second: [1, 2, 1, 2],
  },
  {
    code: "688981",
    trigger: "2026-10-09",
    theme: "科技硬件",
    secondary: "国产替代",
    start: "2026-09-30",
    method: "连续识别 + 字段识别",
    keyword: "国产替代",
    related: ["002475", "000333"],
    generic: ["300059"],
    counts: [3, 4, 6, 6],
    first: [2, 2, 4, 3],
    second: [1, 1, 1, 2],
  },
];
const evidenceDates = [
  "2026-10-14",
  "2026-10-13",
  "2026-10-12",
  "2026-10-09",
  "2026-10-08",
  "2026-10-07",
  "2026-09-30",
  "2026-09-29",
  "2026-09-28",
  "2026-09-25",
];

export function ThemePage(
  props: PageActions & { incoming: DraftRequest | null },
) {
  const [selected, setSelected] = useState("603598");
  const [query, setQuery] = useState("");
  const [fromDate, setFromDate] = useState("2026-09-01");
  const [requestedDate, setRequestedDate] = useState("2026-10-14");
  const [evidenceTab, setEvidenceTab] = useState<"cycle" | "keywords">("cycle");
  const [narrowTab, setNarrowTab] = useState<"evidence" | "related">(
    "evidence",
  );
  const [specificOnly, setSpecificOnly] = useState(false);
  const actualDate =
    evidenceDates.find((date) => date <= requestedDate) ?? null;
  const objects = themeObjects.filter(
    (item) =>
      item.trigger >= fromDate &&
      item.trigger <= requestedDate &&
      `${item.code} ${stocks.find((s) => s.code === item.code)?.name} ${item.theme} ${item.secondary}`.includes(
        query.trim(),
      ),
  );
  const object = objects.find((item) => item.code === selected) ?? objects[0];
  useEffect(() => {
    if (!props.incoming) return;
    const next =
      themeObjects.find((item) => props.incoming?.codes.includes(item.code)) ??
      themeObjects.find((item) => item.theme === props.incoming?.sector);
    if (next) {
      setSelected(next.code);
      setQuery("");
      setFromDate("2026-09-01");
      setRequestedDate(
        props.incoming.date > next.trigger ? props.incoming.date : next.trigger,
      );
      setEvidenceTab("cycle");
    }
  }, [props.incoming?.requestId]);
  const stock = stocks.find((item) => item.code === object?.code);
  const cycleDates = object
    ? evidenceDates
        .filter((date) => date <= object.trigger)
        .slice(0, 4)
        .reverse()
    : [];
  const context = (
    codes: string[] = object ? [object.code] : [],
  ): SourceContext => ({
    date: actualDate ?? requestedDate,
    sector: object?.theme ?? "研究观察",
    codes,
    reason: object
      ? `${object.keyword}；四板触发日 ${object.trigger}，来源：示例涨停资料。`
      : "",
    origin: "theme",
  });
  const copyRelated = async () => {
    if (!object) return;
    try {
      await navigator.clipboard.writeText(
        [...object.related, ...object.generic].join("\n"),
      );
      props.onNotice("已复制合并去重后的联动股票代码");
    } catch {
      props.onNotice("浏览器未允许复制，请在详情中选中代码手动复制");
    }
  };
  return (
    <section
      className="u-page research-page theme-page"
      data-testid="theme-page"
    >
      <header className="u-page-heading">
        <div>
          <div className="u-eyebrow">追溯周期 · 理解关联</div>
          <h1 className="u-page-title">
            题材研究 <span className="research-title-tag">证据驱动</span>
          </h1>
        </div>
        <span className="research-local">
          <Database size={13} /> 示例证据 · 演示研究流程
        </span>
      </header>
      <div className="research-toolbar theme-date-toolbar">
        <CalendarDays size={16} />
        <label>
          触发区间
          <input
            type="date"
            aria-label="题材起始日"
            value={fromDate}
            onChange={(event) => setFromDate(event.target.value)}
          />
        </label>
        <span className="u-muted">至</span>
        <input
          type="date"
          aria-label="题材请求截止日"
          value={requestedDate}
          onChange={(event) => setRequestedDate(event.target.value)}
        />
        <span
          className={`theme-actual-date ${actualDate !== requestedDate ? "fallback" : ""}`}
        >
          <Check size={13} /> 实际截止日{" "}
          <strong>{actualDate ?? "无可用数据"}</strong>
          {actualDate && actualDate !== requestedDate && (
            <span>已回退至最近导入日</span>
          )}
        </span>
        <button
          className="research-text-button"
          onClick={() => {
            setFromDate("2026-09-01");
            setRequestedDate(dates[0]);
          }}
        >
          重置日期
        </button>
      </div>
      <div className="theme-layout">
        <aside className="u-panel theme-object-panel">
          <div className="u-panel-heading">
            <strong>四板观察对象</strong>
            <span className="u-tag">{objects.length}</span>
          </div>
          <div className="theme-object-search">
            <Search size={14} />
            <input
              aria-label="筛选四板观察对象"
              placeholder="股票、代码或题材"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
            {query && (
              <button
                aria-label="清空题材对象搜索"
                onClick={() => setQuery("")}
              >
                <X size={13} />
              </button>
            )}
          </div>
          <div className="theme-object-list">
            {objects.map((item) => {
              const currentStock = stocks.find((s) => s.code === item.code)!;
              return (
                <button
                  className={`theme-object ${item.code === object?.code ? "active" : ""}`}
                  key={`${item.code}-${item.trigger}`}
                  onClick={() => {
                    setSelected(item.code);
                    setNarrowTab("evidence");
                  }}
                >
                  <div className="theme-object-main">
                    <strong>{currentStock.name}</strong>
                    <span className="theme-four-tag">4 板</span>
                  </div>
                  <span className="theme-object-code">
                    {item.code}
                    <span>{item.trigger.slice(5).replace("-", ".")} 触发</span>
                  </span>
                  <div className="theme-object-topic">
                    <span>{item.theme}</span>
                    <ChevronRight size={13} />
                  </div>
                </button>
              );
            })}
            {!objects.length && (
              <div className="u-empty">
                <Search size={24} />
                <span>当前范围没有观察对象</span>
              </div>
            )}
          </div>
          <div className="theme-object-foot">
            <CircleHelp size={14} />
            <p>
              “四日”按已导入日计算。
              <br />
              漏导资料可能影响周期识别。
            </p>
          </div>
        </aside>
        {object && stock ? (
          <>
            <div className="theme-narrow-switch u-segmented">
              <button
                className={narrowTab === "evidence" ? "active" : ""}
                onClick={() => setNarrowTab("evidence")}
              >
                周期与证据
              </button>
              <button
                className={narrowTab === "related" ? "active" : ""}
                onClick={() => setNarrowTab("related")}
              >
                截止日联动
              </button>
            </div>
            <main
              className={`theme-evidence ${narrowTab === "evidence" ? "narrow-active" : ""}`}
            >
              <section className="u-panel theme-summary">
                <div className="theme-summary-top">
                  <div>
                    <div className="research-inline">
                      <span className="theme-symbol-mark">
                        {stock.name.slice(0, 1)}
                      </span>
                      <h2>{stock.name}</h2>
                      <span className="theme-summary-code">{stock.code}</span>
                    </div>
                    <div className="theme-summary-tags">
                      <span className="u-tag teal">{object.theme}</span>
                      <span className="u-tag">{object.method}</span>
                    </div>
                  </div>
                  <button
                    className="u-button small"
                    onClick={() => props.onOpenMarket(stock.code)}
                  >
                    查看行情
                    <ArrowUpRight size={14} />
                  </button>
                </div>
                <div className="theme-cycle-strip">
                  <div>
                    <span>首板日</span>
                    <strong>{cycleDates[0]?.slice(5).replace("-", ".")}</strong>
                  </div>
                  <div className="theme-cycle-track">
                    {cycleDates.map((date, index) => (
                      <span key={date} className={index === 3 ? "last" : ""}>
                        <i />
                        {index + 1} 板
                        <small>{date.slice(5).replace("-", ".")}</small>
                      </span>
                    ))}
                  </div>
                  <div className="theme-trigger-date">
                    <span>四板触发</span>
                    <strong>{object.trigger.slice(5).replace("-", ".")}</strong>
                  </div>
                </div>
                <div className="theme-summary-foot">
                  <span>示例证据 · 历史归属固定于四板周期</span>
                  <BookOpen size={14} />
                </div>
              </section>
              <section className="u-panel theme-evidence-panel">
                <div className="u-panel-heading">
                  <div className="research-tabs">
                    <button
                      className={evidenceTab === "cycle" ? "active" : ""}
                      onClick={() => setEvidenceTab("cycle")}
                    >
                      周期证据
                    </button>
                    <button
                      className={evidenceTab === "keywords" ? "active" : ""}
                      onClick={() => setEvidenceTab("keywords")}
                    >
                      八日关键词
                    </button>
                  </div>
                  <span className="u-tag">可追溯</span>
                </div>
                {evidenceTab === "cycle" ? (
                  <div className="theme-cycle-content">
                    <div className="theme-section-label">
                      <div>
                        <h3>板块梯队</h3>
                        <p>四个已导入日累计次数，同股跨日分别计数</p>
                      </div>
                      <span className="u-tag teal">{object.theme}</span>
                    </div>
                    <div className="theme-metrics-plot">
                      <div className="theme-counts">
                        <div>
                          <span>涨停累计</span>
                          <strong>
                            {object.counts.reduce((a, b) => a + b, 0)}
                            <small> 次</small>
                          </strong>
                        </div>
                        <div>
                          <span>首板累计</span>
                          <strong>
                            {object.first.reduce((a, b) => a + b, 0)}
                            <small> 次</small>
                          </strong>
                        </div>
                        <div>
                          <span>二板累计</span>
                          <strong>
                            {object.second.reduce((a, b) => a + b, 0)}
                            <small> 次</small>
                          </strong>
                        </div>
                      </div>
                      <div className="theme-plot-area">
                        <div className="theme-bar-chart">
                          {cycleDates.map((date, index) => (
                            <div className="theme-bar-column" key={date}>
                              <div className="theme-bar-label">
                                {object.counts[index]}
                              </div>
                              <div className="theme-bar-track">
                                <div
                                  className="theme-bar-total"
                                  style={{
                                    height: `${object.counts[index] * 9}px`,
                                  }}
                                >
                                  <i
                                    style={{
                                      height: `${(object.second[index] / object.counts[index]) * 100}%`,
                                    }}
                                  />
                                </div>
                              </div>
                              <span>{date.slice(5).replace("-", ".")}</span>
                            </div>
                          ))}
                        </div>
                        <div className="theme-chart-legend">
                          <span>
                            <i /> 涨停累计次数
                          </span>
                          <span>
                            <i className="second" /> 其中二板
                          </span>
                        </div>
                      </div>
                    </div>
                    <div className="theme-affiliation-heading">
                      <h3>周期内题材归属</h3>
                      <label>
                        <input
                          type="checkbox"
                          checked={specificOnly}
                          onChange={(event) =>
                            setSpecificOnly(event.target.checked)
                          }
                        />
                        仅具体题材
                      </label>
                    </div>
                    <table className="u-data-table theme-affiliation">
                      <thead>
                        <tr>
                          <th>归属题材</th>
                          <th>出现次数</th>
                          <th>日期覆盖</th>
                        </tr>
                      </thead>
                      <tbody>
                        {[
                          {
                            name: object.theme,
                            count: 4,
                            dates: "全部 4 日",
                            primary: true,
                          },
                          {
                            name: object.secondary,
                            count: 3,
                            dates: "第 2—4 日",
                            primary: false,
                          },
                          ...(!specificOnly
                            ? [
                                {
                                  name: "其他",
                                  count: 1,
                                  dates: "第 1 日",
                                  primary: false,
                                },
                              ]
                            : []),
                        ].map((item) => (
                          <tr key={item.name}>
                            <td>
                              {item.name}
                              {item.primary && (
                                <span className="theme-main-label">主题材</span>
                              )}
                            </td>
                            <td>
                              <div className="theme-occurrence">
                                <span
                                  style={{ width: `${item.count * 18}px` }}
                                />
                                <strong>{item.count}</strong>
                              </div>
                            </td>
                            <td>{item.dates}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                ) : (
                  <div className="theme-keyword-content">
                    <div className="theme-section-label">
                      <div>
                        <h3>每日关键词交集</h3>
                        <p>触发日及之前 7 个已导入日，逐日比较。</p>
                      </div>
                    </div>
                    <div className="theme-keyword-source">
                      <span>源关键词</span>
                      <strong>{object.keyword}</strong>
                      <span>完全与模糊匹配分开标识</span>
                    </div>
                    <table className="u-data-table theme-keyword-table">
                      <thead>
                        <tr>
                          <th>日期 / 股票</th>
                          <th>命中依据</th>
                          <th>匹配</th>
                        </tr>
                      </thead>
                      <tbody>
                        {evidenceDates
                          .filter((date) => date <= object.trigger)
                          .slice(0, 8)
                          .map((date, index) => {
                            const peer = stocks.find(
                              (s) =>
                                s.code ===
                                object.related[index % object.related.length],
                            )!;
                            return (
                              <tr key={date}>
                                <td>
                                  <span className="research-table-sub">
                                    {date.slice(5)}
                                  </span>
                                  <button
                                    className="research-text-button"
                                    onClick={() =>
                                      props.onInspect(peer.code, {
                                        ...context([peer.code]),
                                        date,
                                      })
                                    }
                                  >
                                    {peer.name}
                                  </button>
                                </td>
                                <td>
                                  <span>
                                    {index % 3 === 0
                                      ? object.keyword
                                      : object.keyword.slice(0, 2)}
                                  </span>
                                  <span className="research-table-sub">
                                    {index % 3 === 0
                                      ? "共同板块"
                                      : `源词：${object.keyword}`}
                                  </span>
                                </td>
                                <td>
                                  <span
                                    className={`u-tag ${index % 3 === 0 ? "teal" : "amber"}`}
                                  >
                                    {index % 3 === 0 ? "完全" : "模糊"}
                                  </span>
                                </td>
                              </tr>
                            );
                          })}
                      </tbody>
                    </table>
                    <p className="theme-evidence-note">
                      模糊命中仅展示文本线索，不表示确定的业务关联。
                    </p>
                  </div>
                )}
              </section>
              <div className="theme-bottom-actions">
                <span>
                  <FileText size={14} />
                  把研究依据带入你的预案
                </span>
                <button
                  className="u-button"
                  onClick={() => props.onAddToWatch(context())}
                >
                  <Plus size={14} />
                  加入盯盘
                </button>
                <button
                  className="u-button primary"
                  onClick={() => props.onCreateDraft(context())}
                >
                  <FilePenLine size={14} />
                  引用为预案
                </button>
              </div>
            </main>
            <aside
              className={`u-panel theme-related ${narrowTab === "related" ? "narrow-active" : ""}`}
            >
              <div className="u-panel-heading">
                <div className="research-inline">
                  <Link2 size={15} />
                  <strong>截止日联动</strong>
                </div>
                <span className="u-tag">
                  {actualDate?.slice(5) ?? "无数据"}
                </span>
              </div>
              <div className="theme-related-intro">
                <span>历史归属 × 截止日资料</span>
                <p>从同题材与关键词两条路径，寻找其他观察对象。</p>
              </div>
              <div className="theme-related-scroll">
                {actualDate ? (
                  <>
                    <div className="theme-related-group-heading">
                      <span>
                        <span className="research-sector-dot teal" />
                        同板块
                      </span>
                      <b>{object.related.length} 只</b>
                    </div>
                    <p className="theme-related-description">
                      四板周期具体题材在截止日的其他股票
                    </p>
                    {object.related.map((code, index) => {
                      const peer = stocks.find((s) => s.code === code)!;
                      return (
                        <div className="theme-peer" key={code}>
                          <div className="theme-peer-top">
                            <button
                              className="research-stock-button"
                              onClick={() =>
                                props.onInspect(code, context([code]))
                              }
                            >
                              <strong>{peer.name}</strong>
                              <span>{code}</span>
                            </button>
                            <span className="research-board-tag">
                              {index === 0 ? "2天2板" : "首板"}
                            </span>
                          </div>
                          <div className="theme-peer-bottom">
                            <span>{object.theme}</span>
                            <button
                              aria-label={`将 ${peer.name} 加入盯盘`}
                              onClick={() =>
                                props.onAddToWatch(context([code]))
                              }
                            >
                              <Plus size={14} />
                              盯盘
                            </button>
                          </div>
                        </div>
                      );
                    })}
                    <div className="theme-related-group-heading second">
                      <span>
                        <span className="research-sector-dot amber" />
                        泛板块关键词
                      </span>
                      <b>{object.generic.length} 只</b>
                    </div>
                    <p className="theme-related-description">
                      仅匹配“公告 / 其他”等泛板块资料
                    </p>
                    {object.generic.map((code) => {
                      const peer = stocks.find((s) => s.code === code)!;
                      return (
                        <div className="theme-peer" key={code}>
                          <div className="theme-peer-top">
                            <button
                              className="research-stock-button"
                              onClick={() =>
                                props.onInspect(code, context([code]))
                              }
                            >
                              <strong>{peer.name}</strong>
                              <span>{code}</span>
                            </button>
                            <span className="u-tag amber">模糊</span>
                          </div>
                          <div className="theme-match-explanation">
                            <span>源词 {object.keyword}</span>
                            <ArrowRight size={12} />
                            <span>{object.keyword.slice(0, 2)}应用</span>
                          </div>
                          <div className="theme-peer-bottom">
                            <span>共有核心：{object.keyword.slice(0, 2)}</span>
                            <button
                              aria-label={`将 ${peer.name} 加入盯盘`}
                              onClick={() =>
                                props.onAddToWatch(context([code]))
                              }
                            >
                              <Plus size={14} />
                              盯盘
                            </button>
                          </div>
                        </div>
                      );
                    })}
                  </>
                ) : (
                  <div className="u-empty">
                    <Database size={24} />
                    <span>请求日期之前没有已导入资料</span>
                  </div>
                )}
              </div>
              <div className="theme-related-footer">
                <button
                  className="u-button"
                  disabled={!actualDate}
                  onClick={copyRelated}
                >
                  <Copy size={14} />
                  复制联动代码
                </button>
                <button
                  className="u-button primary"
                  disabled={!actualDate}
                  onClick={() =>
                    props.onAddToWatch(
                      context([...object.related, ...object.generic]),
                    )
                  }
                >
                  <Plus size={14} />
                  全部加入盯盘
                </button>
              </div>
            </aside>
          </>
        ) : (
          <div className="u-panel theme-no-selection">
            <Search size={30} />
            <h2>没有符合条件的四板周期</h2>
            <p>扩大日期范围或清除搜索，查看示例研究对象。</p>
            <button
              className="u-button"
              onClick={() => {
                setQuery("");
                setFromDate("2026-09-01");
                setRequestedDate("2026-10-14");
              }}
            >
              恢复默认范围
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
