import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { access, mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { stripVTControlCharacters } from 'node:util';
import { chromium } from 'playwright';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const artifacts = resolve(root, 'artifacts');
const url = 'http://127.0.0.1:4174';
const dev = process.env.PREVIEW_DEV === '1';
const started = Date.now();
const summary = { mode: dev ? 'development' : 'production', scenarios: [], screenshots: [], pageErrors: [], standalone: dev ? 'checked by production run' : 'not generated' };
if (!dev) await access(resolve(root, 'dist/index.html'));
await mkdir(artifacts, { recursive: true });

// Spawn Vite directly so teardown owns one process, without an npm shell child.
const server = spawn(process.execPath, [resolve(root, 'node_modules/vite/bin/vite.js'), ...(dev ? [] : ['preview']), '--host', '127.0.0.1', '--port', '4174', '--strictPort'], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
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
  await page.getByRole('heading', { name: '引力传媒', exact: true }).waitFor();
  await page.evaluate(() => document.fonts.ready);
}
async function count(locator, expected, message) {
  if (expected === 0) await locator.waitFor({ state: 'hidden' });
  else await locator.first().waitFor({ state: 'visible' });
  assert.equal(await locator.count(), expected, message);
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
    const dialog = page.getByRole('dialog', { name: '查找股票', exact: true });
    await dialog.waitFor();
    const input = page.getByRole('textbox', { name: '搜索股票', exact: true });
    assert.equal(await input.evaluate(el => el === document.activeElement), true);
    await input.fill('600519');
    await count(page.locator('.search-result'), 1);
    assert.match(await page.locator('.search-result').textContent(), /贵州茅台/);
    await page.keyboard.press('Escape');
    await count(dialog, 0);
    assert.equal(await filter.evaluate(el => el === document.activeElement), true, 'Focus returns to the original control');
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
    await page.getByRole('textbox', { name: '搜索股票', exact: true }).fill('001318');
    await count(page.locator('.search-result'), 1);
    assert.match(await page.locator('.search-result').textContent(), /阳光乳业/);
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
