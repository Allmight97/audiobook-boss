/**
 * Lab scenario screenshots + the one browser-level layout assertion.
 *
 * Usage: `bun run lab:shots`
 *
 * Starts the Vite dev server, opens each lab scenario in headless Chromium,
 * saves screenshots to .artifacts/lab-shots/ (gitignored), and asserts the
 * short-window modal scenario keeps its footer actions reachable while the
 * body scrolls — the layout proof jsdom cannot provide (LabIsland.test.ts
 * pins markup only).
 *
 * Prerequisite (one-time per machine): a Playwright Chromium matching the
 * pinned `playwright` devDependency — `bunx playwright install chromium`.
 * Set ABB_LAB_CHROMIUM to an executable path to use a system Chromium
 * instead.
 */
import { mkdirSync } from 'node:fs';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { chromium } from 'playwright';

const PORT = 4517;
const BASE_URL = `http://localhost:${PORT}/lab.html`;
const OUT_DIR = path.join(process.cwd(), '.artifacts/lab-shots');

const SCENARIOS: { id: string; viewport: { width: number; height: number } }[] = [
	{ id: 'chapter-queue', viewport: { width: 1440, height: 900 } },
	{ id: 'chapter-queue-locked', viewport: { width: 1440, height: 900 } },
	{ id: 'empty-queue', viewport: { width: 1440, height: 900 } },
	{ id: 'modal-short', viewport: { width: 1440, height: 500 } },
];

async function waitForServer(url: string, timeoutMs: number): Promise<void> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		try {
			const response = await fetch(url);
			if (response.ok) return;
		} catch {
			// Server not up yet.
		}
		await new Promise((resolve) => setTimeout(resolve, 250));
	}
	throw new Error(`Vite dev server did not answer at ${url} within ${timeoutMs}ms`);
}

async function assertShortModalUsable(page: import('playwright').Page): Promise<void> {
	const body = page.getByTestId('modal-scenario-body');
	const footer = page.getByTestId('modal-scenario-footer');
	const viewportHeight = page.viewportSize()?.height ?? 0;

	const scrolls = await body.evaluate((element) => element.scrollHeight > element.clientHeight);
	if (!scrolls) {
		throw new Error('modal-short: body does not overflow — the scenario proves nothing.');
	}

	const footerBoxBefore = await footer.boundingBox();
	if (!footerBoxBefore || footerBoxBefore.y + footerBoxBefore.height > viewportHeight) {
		throw new Error('modal-short: footer actions are not reachable before scrolling.');
	}

	await page.getByTestId('modal-scenario-row-14').scrollIntoViewIfNeeded();
	const footerBoxAfter = await footer.boundingBox();
	if (!footerBoxAfter || footerBoxAfter.y + footerBoxAfter.height > viewportHeight) {
		throw new Error('modal-short: footer actions became unreachable after scrolling.');
	}
}

async function main(): Promise<void> {
	mkdirSync(OUT_DIR, { recursive: true });

	const vite = spawn('bunx', ['vite', '--port', String(PORT), '--strictPort'], {
		cwd: process.cwd(),
		stdio: 'ignore',
	});

	try {
		await waitForServer(BASE_URL, 30_000);

		const browser = await chromium.launch({
			headless: true,
			executablePath: process.env.ABB_LAB_CHROMIUM || undefined,
		});
		try {
			for (const scenario of SCENARIOS) {
				// Fidelity is judged in dark mode (docs/DECISIONS.md, v3 rebuild).
				const page = await browser.newPage({
					viewport: scenario.viewport,
					colorScheme: 'dark',
				});
				await page.goto(`${BASE_URL}?scenario=${scenario.id}`);
				await page.getByTestId('lab-scenario-stage').waitFor({ timeout: 10_000 });

				if (scenario.id === 'modal-short') {
					await assertShortModalUsable(page);
				}

				const shotPath = path.join(OUT_DIR, `${scenario.id}.png`);
				await page.screenshot({ path: shotPath, fullPage: false });
				console.log(`shot: ${shotPath}`);
				await page.close();
			}
		} finally {
			await browser.close();
		}
		console.log('lab:shots OK — all scenarios rendered; modal-short layout assertion passed.');
	} finally {
		vite.kill();
	}
}

main().catch((error) => {
	console.error(error instanceof Error ? error.message : error);
	process.exit(1);
});
