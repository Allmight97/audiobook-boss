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
	test('rejects headed sessions unless the operator explicitly opts in', async () => {
		const previous = process.env.CONTROLPLANE_ALLOW_HEADED;
		delete process.env.CONTROLPLANE_ALLOW_HEADED;
		try {
			await expect(
				startHarnessSession({ scenario: 'metadata-edit', viewport: 'desktop', headed: true }),
			).rejects.toThrow(/CONTROLPLANE_ALLOW_HEADED=1/);
		} finally {
			if (previous === undefined) {
				delete process.env.CONTROLPLANE_ALLOW_HEADED;
			} else {
				process.env.CONTROLPLANE_ALLOW_HEADED = previous;
			}
		}
	});

	test('persists a session across helper calls and emits local artifacts', async () => {
		const session = await startHarnessSession({ scenario: 'metadata-edit', viewport: 'desktop' });
		expect(session.sessionId).toContain('harness-agent-');
		expect(session.headed).toBe(false);

		const domSummary = await getDomSummary();
		expect(domSummary.summary.counts.buttons).toBeGreaterThan(0);
		expect(domSummary.summary.keyControls.length).toBeGreaterThan(0);
		await access(domSummary.artifactPath, fsConstants.F_OK);
		await access(domSummary.latestArtifactPath, fsConstants.F_OK);

		const screenshot = await captureScreenshot('api-smoke');
		await access(screenshot.screenshotPath, fsConstants.F_OK);
		await access(screenshot.latestScreenshotPath, fsConstants.F_OK);

		const note = await reportText('smoke note');
		expect(await readFile(note.notePath, 'utf8')).toContain('smoke note');
		await access(note.latestNotePath, fsConstants.F_OK);

		await seedScenario('output-preview');
		const review = await runUiReviewChecklist({ viewport: 'desktop' });
		expect(review.viewport).toBe('desktop');
		expect(Array.isArray(review.objectiveFailures)).toBe(true);
		expect(Array.isArray(review.advisoryFindings)).toBe(true);
		await access(review.screenshotPath, fsConstants.F_OK);
		await access(review.latestScreenshotPath, fsConstants.F_OK);
		await access(review.reviewPath, fsConstants.F_OK);
		await access(review.latestReviewPath, fsConstants.F_OK);

		await closeHarnessSession();
		expect(await readHarnessSessionInfo()).toBeNull();
	}, 45_000);
});
