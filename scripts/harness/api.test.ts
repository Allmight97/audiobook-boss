import { afterEach, describe, expect, test } from 'bun:test';
import { access, readFile } from 'node:fs/promises';
import { constants as fsConstants } from 'node:fs';

import {
	captureScreenshot,
	closeHarnessSession,
	getDomSummary,
	readHarnessSessionInfo,
	reportText,
	runUiReviewChecklist,
	seedScenario,
	startHarnessSession,
} from './api';

afterEach(async () => {
	await closeHarnessSession();
});

describe('harness/api', () => {
	test('persists a session across helper calls and emits local artifacts', async () => {
		const session = await startHarnessSession({ scenario: 'metadata-edit', viewport: 'desktop' });
		expect(session.sessionId).toContain('harness-agent-');

		const domSummary = await getDomSummary();
		expect(domSummary.counts.buttons).toBeGreaterThan(0);
		expect(domSummary.keyControls.length).toBeGreaterThan(0);

		const screenshot = await captureScreenshot('api-smoke');
		await access(screenshot.screenshotPath, fsConstants.F_OK);

		const note = await reportText('smoke note');
		expect(await readFile(note.notePath, 'utf8')).toContain('smoke note');

		await seedScenario('output-preview');
		const review = await runUiReviewChecklist({ viewport: 'desktop' });
		expect(review.viewport).toBe('desktop');
		expect(Array.isArray(review.objectiveFailures)).toBe(true);
		expect(Array.isArray(review.advisoryFindings)).toBe(true);
		await access(review.screenshotPath, fsConstants.F_OK);

		await closeHarnessSession();
		expect(await readHarnessSessionInfo()).toBeNull();
	}, 45_000);
});
