import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { access, mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { stripVTControlCharacters } from 'node:util';
import { chromium } from 'playwright';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const artifacts = resolve(root, 'artifacts');
const port = process.env.PREVIEW_PORT ?? '4174';
assert.match(port, /^\d{4,5}$/, 'PREVIEW_PORT must be a valid unprivileged TCP port');
assert.ok(Number(port) >= 1024 && Number(port) <= 65535, 'PREVIEW_PORT must be between 1024 and 65535');
const url = `http://127.0.0.1:${port}`;
const dev = process.env.PREVIEW_DEV === '1';
const started = Date.now();
const summary = { mode: dev ? 'development' : 'production', scenarios: [], screenshots: [], pageErrors: [], standalone: dev ? 'checked by production run' : 'not generated' };
if (!dev) await access(resolve(root, 'dist/index.html'));
await mkdir(artifacts, { recursive: true });

// Spawn Vite directly so teardown owns one process, without an npm shell child.
const server = spawn(process.execPath, [resolve(root, 'node_modules/vite/bin/vite.js'), ...(dev ? [] : ['preview']), '--host', '127.0.0.1', '--port', port, '--strictPort'], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
let serverOutput = '';
let serverError;
server.on('error', error => { serverError = error; });
for (const stream of [server.stdout, server.stderr]) stream.on('data', chunk => { serverOutput = `${serverOutput}${chunk}`.slice(-8000); });
const serverClosed = new Promise(resolveClose => server.once('close', resolveClose));
let browser;
let page;

const pause = ms => new Promise(resolvePause => setTimeout(resolvePause, ms));
async function screenshot(name) {
  const fileName = `${dev ? 'dev-' : ''}${name}.png`;
  await page.screenshot({ path: resolve(artifacts, fileName), animations: 'disabled' });
  summary.screenshots.push(fileName);
}
async function scenario(name, run) {
  const start = Date.now();
  await run();
  summary.scenarios.push({ name, passed: true, durationMs: Date.now() - start });
  console.log(`PASS ${name}`);
}
async function reset(viewport = { width: 1536, height: 960 }) {
  await page.setViewportSize(viewport);
  await page.goto(url, { waitUntil: 'networkidle' });
  await page.getByTestId('market-page').waitFor({ state: 'visible' });
  await page.getByRole('heading', { name: '引力传媒', exact: true }).waitFor();
  await page.evaluate(() => document.fonts.ready);
}
async function count(locator, expected, message) {
  if (expected === 0) await locator.waitFor({ state: 'hidden' });
  else await locator.first().waitFor({ state: 'visible' });
  assert.equal(await locator.count(), expected, message);
}
async function navigate(id) {
  await page.getByTestId(`nav-${id}`).click();
  await page.getByTestId(`${id}-page`).waitFor({ state: 'visible' });
  assert.equal(await page.getByTestId(`nav-${id}`).getAttribute('aria-current'), 'page');
}
async function noPageOverflow() {
  const bounds = await page.evaluate(() => ({ width: innerWidth, scrollWidth: document.documentElement.scrollWidth }));
  assert.ok(bounds.scrollWidth <= bounds.width + 1, `No page-level horizontal overflow: ${JSON.stringify(bounds)}`);
}
async function chartPosition(x = .3, y = .45) {
  const svg = page.getByTestId('market-chart').first().locator('svg');
  const box = await svg.boundingBox();
  assert.ok(box && box.width > 200 && box.height > 100, 'Chart has usable bounds');
  return { svg, box, position: { x: box.width * x, y: box.height * y } };
}
async function layoutBounds() {
  const result = await page.evaluate(() => {
    const chart = document.querySelector('.chart-canvas');
    const bounds = chart.getBoundingClientRect();
    const axes = [...chart.querySelectorAll('text.chart-axis')];
    return {
      viewport: window.innerWidth,
      documentWidth: document.documentElement.scrollWidth,
      canvas: { top: bounds.top, bottom: bounds.bottom, left: bounds.left, right: bounds.right },
      axes: axes.map(axis => {
        const b = axis.getBoundingClientRect();
        return { text: axis.textContent, top: b.top, bottom: b.bottom, left: b.left, right: b.right };
      }),
    };
  });
  assert.ok(result.documentWidth <= result.viewport + 1, 'No page-level horizontal overflow');
  assert.ok(result.axes.some(axis => axis.text === '15:00'), 'Full-day final time tick exists');
  for (const axis of result.axes) {
    assert.ok(axis.top >= result.canvas.top - 2 && axis.bottom <= result.canvas.bottom + 2,
      `Chart axis ${axis.text} is vertically visible: ${JSON.stringify({ axis, canvas: result.canvas })}`);
    assert.ok(axis.left >= result.canvas.left - 2 && axis.right <= result.canvas.right + 2,
      `Chart axis ${axis.text} is horizontally visible`);
  }
}

try {
  const deadline = Date.now() + 15000;
  let ready = false;
  while (Date.now() < deadline) {
    if (serverError || server.exitCode !== null) throw serverError ?? new Error(`Preview server exited: ${serverOutput}`);
    // A service already using the port must not be mistaken for our process.
    if (!stripVTControlCharacters(serverOutput).includes(url)) { await pause(100); continue; }
    try { ready = (await fetch(url, { signal: AbortSignal.timeout(500) })).ok; } catch { /* Startup is bounded by the deadline. */ }
    if (ready) break;
    await pause(100);
  }
  assert.ok(ready, `Preview server did not become ready within 15 seconds: ${serverOutput}`);
  browser = await chromium.launch({ headless: true, ...(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {}) });
  const context = await browser.newContext({ viewport: { width: 1536, height: 960 }, locale: 'zh-CN', reducedMotion: 'reduce' });
  context.on('page', opened => opened.on('pageerror', error => summary.pageErrors.push({ url: opened.url(), message: error.message })));
  page = await context.newPage();
  page.setDefaultTimeout(5000);

  await scenario('desktop viewports keep the full intraday and volume axes visible', async () => {
    for (const viewport of [{ width: 1536, height: 960 }, { width: 1280, height: 800 }, { width: 1920, height: 1080 }]) {
      await reset(viewport);
      await layoutBounds();
      await screenshot(viewport.width === 1536 ? 'overview' : `overview-${viewport.width}`);
    }
  });

  await scenario('stock selection links quotes and charts, and each board retains its selection', async () => {
    await reset();
    await page.getByRole('button', { name: '选择宁德时代', exact: true }).click();
    await page.getByRole('heading', { name: '宁德时代', exact: true }).waitFor();
    assert.match(await page.getByTestId('market-chart').getAttribute('aria-label'), /宁德时代分时/);
    assert.equal(await page.locator('.depth-spread > span').first().textContent(), '268.80');
    await page.getByRole('tab', { name: '短线观察', exact: true }).click();
    await page.getByRole('heading', { name: '阳光乳业', exact: true }).waitFor();
    await page.getByRole('button', { name: '选择赛力斯', exact: true }).click();
    await page.getByRole('tab', { name: 'A 股全景', exact: true }).click();
    await page.getByRole('heading', { name: '宁德时代', exact: true }).waitFor();
    await page.getByRole('tab', { name: '短线观察', exact: true }).click();
    await page.getByRole('heading', { name: '赛力斯', exact: true }).waitFor();
  });

  await scenario('intraday click toggles inspection and movement switches the detail card side', async () => {
    await reset();
    let hit = await chartPosition(.3);
    await hit.svg.click({ position: hit.position });
    const card = page.getByTestId('inspection-card');
    await card.waitFor();
    assert.match(await card.getAttribute('class'), /is-right/);
    await count(page.getByTestId('chart-crosshair'), 1);
    const originalTime = await card.locator('.chart-inspection-heading strong').textContent();
    hit = await chartPosition(.75, .7);
    await page.mouse.move(hit.box.x + hit.position.x, hit.box.y + hit.position.y);
    assert.match(await card.getAttribute('class'), /is-left/);
    assert.notEqual(await card.locator('.chart-inspection-heading strong').textContent(), originalTime);
    await screenshot('intraday-inspection');
    await hit.svg.click({ position: hit.position });
    await count(card, 0);
    await count(page.getByTestId('chart-crosshair'), 0);
    await page.mouse.move(hit.box.x + hit.box.width * .4, hit.box.y + hit.box.height * .5);
    await count(card, 0, 'Hover alone must not reopen inspection');
    // An afternoon minute has a larger index than the entire daily series.
    // Switching modes must clear inspection before indexing daily indicators.
    await hit.svg.click({ position: hit.position });
    await card.waitFor();
    await page.getByRole('tab', { name: '日 K', exact: true }).click();
    await count(card, 0);
    assert.match(await page.getByTestId('market-chart').getAttribute('aria-label'), /日线/);
    await page.getByRole('tab', { name: '分时', exact: true }).click();
    await count(card, 0);
  });

  await scenario('daily inspection opens the selected day in a historical intraday dialog', async () => {
    await reset();
    await page.getByRole('tab', { name: '日 K', exact: true }).click();
    const hit = await chartPosition(.72, .65);
    await hit.svg.click({ position: hit.position });
    const card = page.getByTestId('inspection-card');
    await card.waitFor();
    const date = await card.locator('.chart-inspection-heading strong').textContent();
    assert.match(date, /^2026-\d{2}-\d{2}$/);
    assert.match(await card.textContent(), /开盘.*收盘.*最高.*最低/s);
    await screenshot('daily-inspection');
    await hit.svg.dblclick({ position: hit.position });
    const history = page.getByRole('dialog', { name: '引力传媒 · 历史分时', exact: true });
    await history.waitFor();
    assert.ok((await history.textContent()).includes(date), 'History shows the inspected candle date');
    assert.match(await history.getByTestId('market-chart').getAttribute('aria-label'), /引力传媒分时/);
    await screenshot('historical-intraday');
    await page.keyboard.press('Escape');
    await count(history, 0);
  });

  await scenario('an empty new group imports known symbols once and can be renamed independently', async () => {
    await reset();
    await page.getByRole('button', { name: '新增看板', exact: true }).click();
    await page.getByRole('textbox', { name: '名称', exact: true }).fill('导入验收');
    await page.getByRole('button', { name: '创建看板', exact: true }).click();
    await page.getByRole('tab', { name: '导入验收', exact: true }).waitFor();
    await count(page.locator('.stock-row'), 0);
    await page.getByRole('button', { name: '从文本批量导入', exact: true }).click();
    await page.getByRole('textbox', { name: '股票代码或名称', exact: true }).fill('603598 SH600519 603598 unknown');
    assert.deepEqual(await page.locator('.import-summary b').allTextContents(), ['2', '0', '1', '1']);
    assert.match(await page.locator('.import-warning').textContent(), /unknown/);
    await screenshot('text-import');
    await page.getByRole('button', { name: '添加 2 只股票', exact: true }).click();
    await count(page.locator('.stock-row'), 2);
    await page.getByRole('button', { name: '选择引力传媒', exact: true }).waitFor();
    await page.getByRole('button', { name: '选择贵州茅台', exact: true }).waitFor();
    await page.getByRole('button', { name: '股票分组菜单', exact: true }).click();
    await page.getByRole('button', { name: '重命名分组', exact: true }).click();
    await page.getByRole('textbox', { name: '名称', exact: true }).fill('传媒与消费');
    await page.getByRole('button', { name: '保存名称', exact: true }).click();
    assert.match(await page.locator('.stock-panel > .panel-heading').textContent(), /传媒与消费/);
    await page.getByRole('tab', { name: 'A 股全景', exact: true }).click();
    await count(page.locator('.stock-row'), 12, 'Import must not modify another board');
    await page.getByRole('tab', { name: '导入验收', exact: true }).click();
    await count(page.locator('.stock-row'), 2);
    assert.match(await page.locator('.stock-panel > .panel-heading').textContent(), /传媒与消费/);
  });

  await scenario('components float first, close removes, undo restores, lock prevents docking', async () => {
    await reset();
    await page.getByRole('button', { name: '添加组件', exact: true }).first().click();
    await page.getByRole('dialog', { name: '搭建你的看板', exact: true }).waitFor();
    await screenshot('components');
    await page.locator('.component-card').filter({ hasText: '关键数据' }).click();
    const floating = page.getByTestId('floating-panel');
    await count(floating, 1);
    await count(page.getByTestId('docked-metrics'), 0);
    await page.getByRole('button', { name: '关闭独立关键数据', exact: true }).click();
    await count(floating, 0);
    await count(page.getByTestId('docked-metrics'), 0, 'Close must not insert into the board');
    await page.getByRole('status').getByRole('button', { name: '撤销', exact: true }).click();
    await count(floating, 1);
    await page.getByRole('button', { name: '自由布局', exact: true }).click();
    assert.equal(await floating.getByRole('button', { name: '插入面板', exact: true }).isDisabled(), true);
    await page.getByRole('button', { name: '布局已锁定', exact: true }).click();
    await floating.getByRole('button', { name: '插入面板', exact: true }).click();
    await page.getByRole('button', { name: '右侧', exact: true }).click();
    await count(floating, 0);
    await count(page.getByTestId('docked-metrics'), 1);
    const chart = await page.getByTestId('docked-chart').boundingBox();
    const metrics = await page.getByTestId('docked-metrics').boundingBox();
    assert.ok(metrics.x >= chart.x + chart.width - 1, 'Right insertion has the requested placement');
    await screenshot('docked-components');
    await page.getByRole('button', { name: '弹出关键数据', exact: true }).click();
    await count(floating, 1);
    await count(page.getByTestId('docked-metrics'), 0);
    await screenshot('floating-component');
  });

  await scenario('Ctrl K searches and Escape restores focus to the originating control', async () => {
    await reset();
    const filter = page.getByRole('textbox', { name: '筛选当前分组', exact: true });
    await filter.focus();
    await page.keyboard.press('Control+k');
    const dialog = page.getByRole('dialog', { name: '全局搜索', exact: true });
    await dialog.waitFor();
    const input = dialog.getByPlaceholder('搜索股票、代码或页面', { exact: true });
    assert.equal(await input.evaluate(el => el === document.activeElement), true);
    await input.fill('600519');
    await count(dialog.locator('.global-stock-result'), 1);
    assert.match(await dialog.locator('.global-stock-result').textContent(), /贵州茅台/);
    await input.dispatchEvent('keydown', { key: 'Escape', isComposing: true, keyCode: 229, bubbles: true });
    await dialog.waitFor({ state: 'visible' });
    await page.keyboard.press('Escape');
    await count(dialog, 0);
    assert.equal(await filter.evaluate(el => el === document.activeElement), true, 'Focus returns to the original control');
  });

  await scenario('five work pages share navigation without losing the market selection', async () => {
    await reset();
    await page.getByRole('button', { name: '选择宁德时代', exact: true }).click();
    for (const id of ['watch', 'limit', 'theme', 'journal']) {
      await navigate(id);
      assert.equal(await page.getByTestId('market-page').isVisible(), false);
      await noPageOverflow();
      await screenshot(`workspace-${id}`);
    }
    await page.setViewportSize({ width: 1280, height: 800 });
    for (const id of ['watch', 'limit', 'theme', 'journal']) {
      await navigate(id);
      await noPageOverflow();
      if (id === 'watch' || id === 'journal') await screenshot(`workspace-${id}-1280`);
    }
    await page.setViewportSize({ width: 1536, height: 960 });
    await navigate('market');
    await page.getByTestId('market-page').getByRole('heading', { name: '宁德时代', exact: true }).waitFor();
    await page.keyboard.press('Control+k');
    const search = page.getByRole('dialog', { name: '全局搜索', exact: true });
    await search.getByPlaceholder('搜索股票、代码或页面', { exact: true }).fill('题材');
    await page.keyboard.press('Enter');
    await page.getByTestId('theme-page').waitFor({ state: 'visible' });
    await count(search, 0);
  });

  await scenario('watch monitoring and unread alerts survive page switches and open the linked stock', async () => {
    await reset();
    await navigate('watch');
    const watch = page.getByTestId('watch-page');
    await count(watch.getByTestId('watch-group'), 4);
    await count(watch.getByTestId('watch-alert'), 3);
    assert.match(await watch.locator('.watch-unread-count').textContent(), /2 条未读/);
    await watch.getByRole('button', { name: '暂停监控', exact: true }).click();
    assert.equal(await watch.getByRole('button', { name: '演示异动', exact: true }).isDisabled(), true);
    await navigate('theme');
    assert.match(await page.locator('.unified-status').textContent(), /演示监控已暂停/);
    await navigate('watch');
    await watch.getByRole('button', { name: '继续监控', exact: true }).click();
    await watch.getByRole('button', { name: '演示异动', exact: true }).click();
    await count(watch.getByTestId('watch-alert'), 4);
    assert.match(await watch.locator('.watch-unread-count').textContent(), /3 条未读/);
    await watch.getByTestId('watch-alert').first().click();
    const inspection = page.getByRole('dialog', { name: '个股速览', exact: true });
    await inspection.getByRole('heading', { name: '引力传媒', exact: true }).waitFor();
    await page.keyboard.press('Escape');
    await count(inspection, 0);
    assert.match(await watch.locator('.watch-unread-count').textContent(), /2 条未读/);
    await watch.getByRole('button', { name: '全部标为已读', exact: true }).click();
    assert.match(await watch.locator('.watch-unread-count').textContent(), /0 条未读/);
    await navigate('limit');
    await navigate('watch');
    await count(watch.getByTestId('watch-alert'), 4, 'Reading an alert does not delete the record');
    await watch.getByRole('button', { name: '打开 阳光乳业 行情', exact: true }).click();
    await page.getByTestId('market-page').waitFor({ state: 'visible' });
    await page.getByTestId('market-page').getByRole('heading', { name: '阳光乳业', exact: true }).waitFor();
    await count(page.getByRole('dialog', { name: '个股速览', exact: true }), 0);
  });

  await scenario('global search cannot bypass a group editor and resumes when the editor closes', async () => {
    await reset();
    await navigate('watch');
    const watch = page.getByTestId('watch-page');
    await watch.getByRole('button', { name: '管理分组 传媒 · 情绪观察', exact: true }).click();
    const editor = page.getByRole('dialog', { name: '管理分组', exact: true });
    const name = editor.getByLabel('分组名称', { exact: true });
    await name.fill('尚未提交的分组名称');
    await page.keyboard.press('Control+k');
    const search = page.getByRole('dialog', { name: '全局搜索', exact: true });
    await count(search, 0, 'Global navigation must not leave an editable group snapshot open on another page');
    assert.equal(await editor.isVisible(), true);
    assert.equal(await name.inputValue(), '尚未提交的分组名称');
    assert.equal(await page.getByTestId('nav-watch').getAttribute('aria-current'), 'page');
    assert.match(await page.locator('.unified-toast').textContent(), /请先完成或关闭当前弹窗/);
    await editor.getByRole('button', { name: '取消', exact: true }).click();
    await count(editor, 0);
    await watch.getByRole('button', { name: '管理分组 传媒 · 情绪观察', exact: true }).waitFor();
    await page.keyboard.press('Control+k');
    await search.waitFor();
    await search.getByPlaceholder('搜索股票、代码或页面', { exact: true }).fill('题材');
    await page.keyboard.press('Enter');
    await page.getByTestId('theme-page').waitFor({ state: 'visible' });
    await count(search, 0);
  });

  await scenario('historical research retains its date, sector and evidence through stock inspection into a draft', async () => {
    await reset();
    await navigate('limit');
    const limit = page.getByTestId('limit-page');
    await limit.getByRole('button', { name: '2026-10-08 AI 应用 3 条', exact: true }).click();
    await limit.locator('.research-stock-button').filter({ hasText: '引力传媒' }).click();
    const inspection = page.getByRole('dialog', { name: '个股速览', exact: true });
    await inspection.getByRole('heading', { name: '引力传媒', exact: true }).waitFor();
    await inspection.getByRole('button', { name: '引用到复盘', exact: true }).click();
    const handoff = page.getByRole('dialog', { name: '引用为复盘草稿', exact: true });
    await handoff.waitFor();
    assert.equal(await handoff.getByLabel('草稿业务日期', { exact: true }).inputValue(), '2026-10-08');
    assert.match(await handoff.locator('.source-summary').textContent(), /来自涨停.*AI 应用.*资料日期 2026-10-08/s);
    assert.match(await handoff.locator('.source-summary').textContent(), /AI 营销 \+ 数字传媒 \+ 内容生产/);
    await handoff.getByRole('button', { name: '创建草稿', exact: true }).click();
    const editor = page.getByTestId('journal-editor');
    await editor.waitFor();
    assert.equal(await page.getByTestId('journal-page').getByLabel('复盘日期', { exact: true }).inputValue(), '2026-10-08');
    assert.equal(await editor.getByLabel('板块名称', { exact: true }).inputValue(), 'AI 应用');
    assert.match(await editor.getByLabel('赚钱效应与判断依据', { exact: true }).inputValue(), /资料 2026-10-08.*AI 营销 \+ 数字传媒 \+ 内容生产/s);
    assert.match(await editor.locator('.journal-editor-context').textContent(), /涨停引用/);
    assert.match(await editor.locator('.journal-editor-stocks').textContent(), /引力传媒/);
    await editor.getByRole('button', { name: '保留草稿', exact: true }).click();
  });

  await scenario('research uses AND filters and appends only missing stocks to the chosen watch group', async () => {
    await reset();
    await navigate('limit');
    const limit = page.getByTestId('limit-page');
    await limit.getByRole('combobox', { name: '搜索字段', exact: true }).selectOption('sector');
    await limit.getByRole('textbox', { name: '涨停搜索条件', exact: true }).fill('AI 应用');
    await page.keyboard.press('Enter');
    await limit.getByRole('combobox', { name: '搜索字段', exact: true }).selectOption('date');
    await limit.getByRole('textbox', { name: '涨停搜索条件', exact: true }).fill('2026-10-14');
    await page.keyboard.press('Enter');
    await count(limit.locator('.limit-table tbody tr'), 3);
    assert.match(await limit.locator('.limit-search-summary').textContent(), /2 个条件同时满足，共 3 条资料/);
    const results = await limit.locator('.limit-table tbody tr').allTextContents();
    for (const result of results) assert.match(result, /10-14.*AI 应用/s);
    await limit.getByRole('button', { name: '清空条件', exact: true }).click();
    await limit.getByRole('button', { name: '加入盯盘', exact: true }).click();
    const handoff = page.getByRole('dialog', { name: '加入盯盘分组', exact: true });
    await handoff.getByRole('combobox', { name: '目标盯盘分组', exact: true }).selectOption('media');
    assert.match(await handoff.locator('.source-summary').textContent(), /来自涨停.*AI 应用.*2026-10-14/s);
    assert.match(await handoff.locator('.handoff-total').textContent(), /新增 1 只.*跳过 2 只重复股票/s);
    await screenshot('research-watch-handoff');
    await handoff.getByRole('button', { name: '确认加入', exact: true }).click();
    const watch = page.getByTestId('watch-page');
    await watch.waitFor({ state: 'visible' });
    const media = watch.getByTestId('watch-group').filter({ hasText: '传媒 · 情绪观察' });
    await count(media.locator('.watch-stock-row'), 4);
    const codes = await media.locator('.watch-stock-name small').allTextContents();
    for (const code of ['603598', '001318', '300059', '002475']) assert.ok(codes.some(text => text.includes(code)), `Watch group retains ${code}`);
    const technology = watch.getByTestId('watch-group').filter({ hasText: '科技 · 趋势跟踪' });
    await count(technology.locator('.watch-stock-row'), 3, 'Appending to one group does not change another');
    await navigate('limit');
    await limit.getByRole('button', { name: '加入盯盘', exact: true }).click();
    await handoff.waitFor();
    assert.match(await handoff.locator('.handoff-total').textContent(), /新增 0 只.*跳过 3 只重复股票/s);
    assert.equal(await handoff.getByRole('button', { name: '确认加入', exact: true }).isDisabled(), true);
    await handoff.getByRole('button', { name: '取消', exact: true }).click();
    await limit.getByRole('button', { name: '查看题材', exact: true }).click();
    await page.getByTestId('theme-page').waitFor({ state: 'visible' });
    assert.equal(await page.getByTestId('theme-page').getByLabel('题材请求截止日', { exact: true }).inputValue(), '2026-10-14');
  });

  await scenario('theme sources become an independent dated draft, survive editing, and persist after reading closes', async () => {
    await reset();
    await navigate('theme');
    const theme = page.getByTestId('theme-page');
    await theme.getByLabel('题材请求截止日', { exact: true }).fill('2026-10-16');
    assert.match(await theme.locator('.theme-actual-date').textContent(), /2026-10-14.*已回退至最近导入日/s);
    await theme.getByRole('button', { name: '引用为预案', exact: true }).click();
    const handoff = page.getByRole('dialog', { name: '引用为复盘草稿', exact: true });
    assert.equal(await handoff.getByLabel('草稿业务日期', { exact: true }).inputValue(), '2026-10-14');
    assert.match(await handoff.locator('.source-summary').textContent(), /来自题材.*AI 应用.*资料日期 2026-10-14/s);
    await handoff.getByLabel('草稿业务日期', { exact: true }).fill('2026-10-15');
    await handoff.getByRole('button', { name: '创建草稿', exact: true }).click();
    const journal = page.getByTestId('journal-page');
    const editor = page.getByTestId('journal-editor');
    await editor.waitFor();
    assert.equal(await journal.getByLabel('复盘日期', { exact: true }).inputValue(), '2026-10-15');
    assert.match(await editor.locator('.journal-editor-context').textContent(), /2026-10-15.*题材引用/s);
    assert.equal(await editor.getByLabel('板块名称', { exact: false }).inputValue(), 'AI 应用');
    assert.match(await editor.getByLabel('赚钱效应与判断依据', { exact: true }).inputValue(), /资料 2026-10-14/);
    assert.match(await editor.locator('.journal-editor-stocks').textContent(), /引力传媒/);
    await editor.getByRole('button', { name: '加入当日预案', exact: true }).click();
    await editor.getByRole('alert').waitFor();
    await count(journal.locator('.journal-plans-scroll').getByTestId('journal-plan'), 0, 'An incomplete draft does not become a plan');
    await editor.getByLabel('买点手法', { exact: true }).fill('分歧承接验收');
    await editor.getByLabel('买入条件', { exact: true }).fill('缩量回踩后，观察放量承接与板块同步。');
    await screenshot('journal-draft');
    await editor.getByRole('button', { name: '保留草稿', exact: true }).click();
    await journal.getByLabel('复盘日期', { exact: true }).fill('2026-10-14');
    await count(journal.locator('.journal-plans-scroll').getByTestId('journal-plan'), 2, 'Existing plans on another date remain untouched');
    await journal.getByLabel('复盘日期', { exact: true }).fill('2026-10-15');
    await journal.locator('.journal-drafts').getByRole('button', { name: 'AI 应用', exact: true }).click();
    assert.equal(await editor.getByLabel('买点手法', { exact: true }).inputValue(), '分歧承接验收');
    await editor.getByRole('button', { name: '加入当日预案', exact: true }).click();
    await count(editor, 0);
    const plans = journal.locator('.journal-plans-scroll').getByTestId('journal-plan');
    await count(plans, 1);
    assert.match(await plans.textContent(), /AI 应用.*分歧承接验收.*缩量回踩后/s);
    await journal.getByRole('button', { name: '打开复盘阅读浮窗', exact: true }).click();
    const reading = page.getByTestId('journal-reading-window');
    await reading.waitFor();
    await count(reading.getByTestId('journal-plan'), 1);
    await reading.getByRole('textbox', { name: '浮窗盘中记录', exact: true }).fill('14:40 验证阅读浮窗共享记录，关闭只关闭视图。');
    await screenshot('journal-reading');
    await reading.getByRole('button', { name: '关闭复盘阅读浮窗', exact: true }).click();
    await count(reading, 0);
    await count(plans, 1, 'Closing the reading view never deletes the document');
    assert.match(await journal.getByRole('textbox', { name: '盘中记录', exact: true }).inputValue(), /14:40 验证阅读浮窗共享记录/);
    await journal.getByRole('button', { name: '立即保存复盘', exact: true }).click();
    assert.match(await journal.locator('.journal-save-state').textContent(), /已保存至浏览器/);
    await page.reload({ waitUntil: 'networkidle' });
    await navigate('journal');
    await journal.getByLabel('复盘日期', { exact: true }).fill('2026-10-15');
    await count(plans, 1);
    assert.match(await journal.getByRole('textbox', { name: '盘中记录', exact: true }).inputValue(), /14:40 验证阅读浮窗共享记录/);
  });

  const standalone = resolve(root, 'dist/tydaro-preview.html');
  let standaloneExists = true;
  try { await access(standalone); } catch { standaloneExists = false; }
  if (standaloneExists && !dev) await scenario('standalone HTML retains search without fetching scripts or styles', async () => {
    const isolated = await browser.newContext({ viewport: { width: 1536, height: 960 }, locale: 'zh-CN' });
    isolated.on('page', opened => opened.on('pageerror', error => summary.pageErrors.push({ url: opened.url(), message: error.message })));
    page = await isolated.newPage();
    page.setDefaultTimeout(5000);
    const fileUrl = pathToFileURL(standalone).href;
    const httpUrl = `${url}/tydaro-preview.html`;
    const assetRequests = [];
    await isolated.route('**/*', route => {
      if ([fileUrl, httpUrl].includes(route.request().url())) return route.continue();
      assetRequests.push(route.request().url());
      return route.abort();
    });
    let fileResult = 'passed';
    try { await page.goto(fileUrl, { waitUntil: 'load' }); }
    catch (error) {
      if (!String(error).includes('ERR_BLOCKED_BY_ADMINISTRATOR')) throw error;
      // Managed Chromium may disallow file URLs. Record the limitation and
      // verify the same inline bundle over HTTP without allowing dependencies.
      fileResult = 'blocked by browser administrator policy; verified over HTTP';
      await page.close();
      page = await isolated.newPage();
      page.setDefaultTimeout(5000);
      await page.goto(httpUrl, { waitUntil: 'load' });
    }
    await page.getByRole('heading', { name: '引力传媒', exact: true }).waitFor();
    await page.keyboard.press('Control+k');
    await page.getByRole('dialog', { name: '全局搜索', exact: true })
      .getByPlaceholder('搜索股票、代码或页面', { exact: true }).fill('001318');
    await count(page.locator('.global-stock-result'), 1);
    assert.match(await page.locator('.global-stock-result').textContent(), /阳光乳业/);
    await page.keyboard.press('Enter');
    await page.getByTestId('market-page').getByRole('heading', { name: '阳光乳业', exact: true }).waitFor();
    await navigate('journal');
    await navigate('theme');
    assert.deepEqual(assetRequests, [], 'Standalone HTML must not request separate assets');
    summary.standalone = { fileUrl: fileResult, dependencyRequests: assetRequests };
  });
  assert.deepEqual(summary.pageErrors, [], 'No browser page errors');
  summary.passed = true;
} catch (error) {
  summary.passed = false;
  summary.error = error.stack ?? String(error);
  if (page && !page.isClosed()) await screenshot('failure').catch(() => {});
  throw error;
} finally {
  summary.durationMs = Date.now() - started;
  await writeFile(resolve(artifacts, dev ? 'summary-dev.json' : 'summary.json'), `${JSON.stringify(summary, null, 2)}\n`);
  await browser?.close().catch(error => console.error(`Browser cleanup: ${error.message}`));
  if (server.exitCode === null) {
    server.kill('SIGTERM');
    const force = setTimeout(() => server.kill('SIGKILL'), 5000);
    await serverClosed;
    clearTimeout(force);
  }
}
console.log(`Preview smoke passed: ${summary.scenarios.length} scenarios, ${summary.screenshots.length} screenshots in ${(summary.durationMs / 1000).toFixed(1)}s.`);
