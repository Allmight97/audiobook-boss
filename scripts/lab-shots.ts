/**
 * Lab scenario screenshots + browser-level interaction/layout assertions.
 *
 * Usage: `bun run lab:shots`
 *
 * Starts the Vite dev server, opens each lab scenario in headless Chromium,
 * saves screenshots to .artifacts/lab-shots/ (gitignored), and asserts:
 * - the short-window modal scenario keeps its footer actions reachable while
 *   the body scrolls — the layout proof jsdom cannot provide (LabIsland.test.ts
 *   pins markup only).
 * - the chapter-queue drag reorder lands where the drag-over indicator
 *   promised (pins the F1 fix) — real pointer drag, not simulable in jsdom.
 * - the modal-escape scenario's Escape key closes the dialog through the real
 *   visibility-transition window regardless of focus (pins the PR-2 Escape
 *   fix) — jsdom has no CSS transitions or WebKit-style focus timing.
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

/** Reads the visible row order (title text) from the chapter-queue table, top to bottom. */
async function readQueueRowTitles(page: import('playwright').Page): Promise<string[]> {
	return page
		.locator('tr[data-file-index]')
		.evaluateAll((rows) =>
			rows.map((row) => row.querySelector('.file-list-file-name')?.textContent?.trim() ?? ''),
		);
}

/** Presses the grip past the reorder threshold, then moves to a target row's edge. */
async function dragGripTo(
	page: import('playwright').Page,
	fromIndex: number,
	target: { x: number; y: number },
): Promise<void> {
	const gripBox = await page
		.locator(`tr[data-file-index="${fromIndex}"] .file-list-reorder-grip`)
		.boundingBox();
	if (!gripBox) throw new Error(`queue-drag: grip for row ${fromIndex} has no layout box.`);
	const gripX = gripBox.x + gripBox.width / 2;
	const gripY = gripBox.y + gripBox.height / 2;

	await page.mouse.move(gripX, gripY);
	await page.mouse.down();
	// Move past the 4px engage threshold before hit-testing the target row.
	await page.mouse.move(gripX, gripY + 10, { steps: 3 });
	await page.mouse.move(target.x, target.y, { steps: 5 });
}

/**
 * Drives a real pointer drag on the chapter-queue scenario and asserts the
 * drop lands exactly where the drag-over indicator promised — pins the F1
 * fix (drop-position truth). The drag implementation is pointer-events-based
 * (src/ui/fileList/events.ts, midpoint hit-testing via getBoundingClientRect),
 * so Playwright mouse.down/move/up drives it directly; no OS drag layer.
 */
async function assertQueueDragLandsAtIndicator(page: import('playwright').Page): Promise<void> {
	const initialTitles = await readQueueRowTitles(page);
	if (initialTitles.length === 0) {
		throw new Error('queue-drag: chapter-queue scenario rendered no rows.');
	}

	const targetIndex = 3;
	const targetBox = await page.locator(`tr[data-file-index="${targetIndex}"]`).boundingBox();
	if (!targetBox) throw new Error(`queue-drag: target row ${targetIndex} has no layout box.`);
	const targetX = targetBox.x + targetBox.width / 2;
	const targetUpperY = targetBox.y + targetBox.height * 0.25;
	const targetLowerY = targetBox.y + targetBox.height * 0.75;

	await dragGripTo(page, 0, { x: targetX, y: targetUpperY });
	const targetRow = page.locator(`tr[data-file-index="${targetIndex}"]`);
	const hasTopIndicator = await targetRow.evaluate((row) => row.classList.contains('drag-over'));
	if (!hasTopIndicator) {
		throw new Error('queue-drag: expected drag-over (top indicator) while hovering the upper half.');
	}

	await page.mouse.move(targetX, targetLowerY, { steps: 5 });
	const hasBottomIndicator = await targetRow.evaluate((row) =>
		row.classList.contains('drag-over-bottom'),
	);
	if (!hasBottomIndicator) {
		throw new Error('queue-drag: expected drag-over-bottom while hovering the lower half.');
	}

	await page.mouse.up();

	const finalTitles = await readQueueRowTitles(page);
	const expectedTitles = [...initialTitles];
	const [moved] = expectedTitles.splice(0, 1);
	expectedTitles.splice(targetIndex, 0, moved);
	if (JSON.stringify(finalTitles) !== JSON.stringify(expectedTitles)) {
		throw new Error(
			`queue-drag: final order did not match the indicator's promise.\nexpected: ${expectedTitles.join(' | ')}\nactual:   ${finalTitles.join(' | ')}`,
		);
	}
}

/** Dropping on the lower half of the last row must append the dragged row at the end. */
async function assertQueueDragAppendsAfterLast(page: import('playwright').Page): Promise<void> {
	const initialTitles = await readQueueRowTitles(page);
	const lastIndex = initialTitles.length - 1;
	if (lastIndex < 1) throw new Error('queue-drag: not enough rows to prove append-after-last.');

	const lastBox = await page.locator(`tr[data-file-index="${lastIndex}"]`).boundingBox();
	if (!lastBox) throw new Error(`queue-drag: last row ${lastIndex} has no layout box.`);
	const lastX = lastBox.x + lastBox.width / 2;
	const lastLowerY = lastBox.y + lastBox.height * 0.75;

	await dragGripTo(page, 0, { x: lastX, y: lastLowerY });
	await page.mouse.up();

	const finalTitles = await readQueueRowTitles(page);
	if (finalTitles.length !== initialTitles.length || finalTitles[finalTitles.length - 1] !== initialTitles[0]) {
		throw new Error(
			`queue-drag: expected the dragged row last.\nexpected last: ${initialTitles[0]}\nactual last:   ${finalTitles[finalTitles.length - 1]}`,
		);
	}
}

async function waitForModalEscapeOpenState(
	page: import('playwright').Page,
	open: boolean,
): Promise<void> {
	await page.waitForFunction(
		(wantOpen) =>
			document.querySelector('[data-testid="modal-escape-backdrop"]')?.classList.contains('open') ===
			wantOpen,
		open,
		{ timeout: 2_000 },
	);
}

/**
 * Drives Escape through the real ModalController/CSS visibility transition
 * on the modal-escape scenario — pins the PR-2 fix that Escape works
 * regardless of where focus sits (document-level, capture-phase handling in
 * src/lib/ui/modal.svelte.ts). Chromium cannot reproduce the exact WebKit
 * focus-refusal timing this fix addresses; that's accepted — the proof
 * verifies the invariant the fix guarantees, not the original repro.
 */
async function assertModalEscapeClosesRegardlessOfFocus(
	page: import('playwright').Page,
): Promise<void> {
	// Case 1: open, then press Escape immediately with no waits in between —
	// drives the keypress through the visibility-transition window.
	await page.getByTestId('modal-escape-open').click();
	await page.keyboard.press('Escape');
	await waitForModalEscapeOpenState(page, false);

	// Case 2: reopen, force focus OUTSIDE the dialog, then Escape. A click on
	// a non-focusable area is not guaranteed to move focus out in Chromium,
	// which would let this proof pass against a container-scoped listener —
	// the exact design this proof exists to reject. Blur explicitly and
	// assert focus really is outside before pressing the key.
	await page.getByTestId('modal-escape-open').click();
	await waitForModalEscapeOpenState(page, true);
	await page.getByTestId('modal-escape-heading').click();
	await page.evaluate(() => {
		(document.activeElement as HTMLElement | null)?.blur();
	});
	const focusOutsideDialog = await page
		.getByTestId('modal-escape-dialog')
		.evaluate((dialog) => !dialog.contains(document.activeElement));
	if (!focusOutsideDialog) {
		throw new Error('modal-escape: could not move focus outside the dialog for case 2.');
	}
	await page.keyboard.press('Escape');
	await waitForModalEscapeOpenState(page, false);

	// Case 3: reopen, press Tab — focus must land/stay inside the dialog.
	await page.getByTestId('modal-escape-open').click();
	await waitForModalEscapeOpenState(page, true);
	await page.keyboard.press('Tab');
	const focusInsideDialog = await page
		.getByTestId('modal-escape-dialog')
		.evaluate((dialog) => dialog.contains(document.activeElement));
	if (!focusInsideDialog) {
		throw new Error('modal-escape: focus left the dialog after Tab.');
	}

	await page.getByTestId('modal-escape-close').click();
	await waitForModalEscapeOpenState(page, false);
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

			// Interaction proofs run on their own fresh pages (dedicated navigations,
			// not the screenshot pages above) so mutating drag/keyboard state cannot
			// bleed into the chapter-queue or modal-short screenshots.
			const queueDragPage = await browser.newPage({
				viewport: { width: 1440, height: 900 },
				colorScheme: 'dark',
			});
			await queueDragPage.goto(`${BASE_URL}?scenario=chapter-queue`);
			await queueDragPage.getByTestId('lab-scenario-stage').waitFor({ timeout: 10_000 });
			await assertQueueDragLandsAtIndicator(queueDragPage);
			console.log('proof: chapter-queue drag lands at the indicator (F1)');
			await queueDragPage.close();

			const queueDragAppendPage = await browser.newPage({
				viewport: { width: 1440, height: 900 },
				colorScheme: 'dark',
			});
			await queueDragAppendPage.goto(`${BASE_URL}?scenario=chapter-queue`);
			await queueDragAppendPage.getByTestId('lab-scenario-stage').waitFor({ timeout: 10_000 });
			await assertQueueDragAppendsAfterLast(queueDragAppendPage);
			console.log('proof: chapter-queue drag appends after the last row (F1)');
			await queueDragAppendPage.close();

			const modalEscapePage = await browser.newPage({
				viewport: { width: 1440, height: 900 },
				colorScheme: 'dark',
			});
			await modalEscapePage.goto(`${BASE_URL}?scenario=modal-escape`);
			await modalEscapePage.getByTestId('lab-scenario-stage').waitFor({ timeout: 10_000 });
			await assertModalEscapeClosesRegardlessOfFocus(modalEscapePage);
			console.log('proof: modal-escape closes regardless of focus (PR-2)');
			await modalEscapePage.close();
		} finally {
			await browser.close();
		}
		console.log(
			'lab:shots OK — all scenarios rendered; modal-short layout, queue-drag, and modal-escape assertions passed.',
		);
	} finally {
		vite.kill();
	}
}

main().catch((error) => {
	console.error(error instanceof Error ? error.message : error);
	process.exit(1);
});
