import type { Page } from 'playwright';

import { resetHarnessState } from './shared';
import type {
	HarnessScenario,
	HarnessScenarioId,
	HarnessScenarioVerifyCheck,
} from '../../src/harness/scenarios';

export type HarnessScenarioCheckStatus = 'passed' | 'failed' | 'not-run';

export type HarnessScenarioCheckResult = {
	id: string;
	label: string;
	status: HarnessScenarioCheckStatus;
	detail?: string;
};

export class HarnessScenarioVerificationError extends Error {
	constructor(
		message: string,
		readonly checkResults: HarnessScenarioCheckResult[],
	) {
		super(message);
		this.name = 'HarnessScenarioVerificationError';
	}
}

function requireScenarioCheck(
	scenario: HarnessScenario,
	checkId: HarnessScenarioCheckResult['id'],
): HarnessScenarioVerifyCheck {
	const check = scenario.verifyChecks.find((entry) => entry.id === checkId);
	if (!check) {
		throw new Error(`Scenario ${scenario.id} is missing verify check ${checkId}`);
	}
	return check;
}

async function runScenarioCheck(
	results: HarnessScenarioCheckResult[],
	check: HarnessScenarioVerifyCheck,
	run: () => Promise<void>,
): Promise<void> {
	try {
		await run();
		results.push({
			id: check.id,
			label: check.label,
			status: 'passed',
		});
	} catch (error) {
		const detail = error instanceof Error ? error.message : String(error);
		const failureResults = [
			...results,
			{
				id: check.id,
				label: check.label,
				status: 'failed' as const,
				detail,
			},
		];
		throw new HarnessScenarioVerificationError(detail, failureResults);
	}
}

async function ensureTitleValue(page: Page, expectedTitle: string): Promise<void> {
	await page.locator('#meta-title').evaluate((node, title) => {
		if (!(node instanceof HTMLInputElement)) {
			throw new Error('Expected metadata title input to be an HTMLInputElement.');
		}
		if (node.value !== title) {
			throw new Error(`Expected metadata title to be ${title}, received ${node.value}`);
		}
	}, expectedTitle);
}

export async function seedMetadataScenario(
	page: Page,
	scenario: HarnessScenario,
): Promise<HarnessScenarioCheckResult[]> {
	await resetHarnessState(page);
	const results: HarnessScenarioCheckResult[] = [];

	await runScenarioCheck(results, scenario.verifyChecks[0], async () => {
		await page.click('[data-testid="metadata-lookup-btn"]');
		await page.locator('[data-testid="metadata-lookup-modal"]').waitFor();
		await page.fill('[data-testid="metadata-lookup-query"]', 'Dune');
		await page.click('[data-testid="metadata-lookup-search-btn"]');
		await page.locator('#metadata-lookup-status').filter({ hasText: 'Found 1 results.' }).waitFor();
		await page.click('#metadata-lookup-results button[data-index="0"]');
		await page
			.locator('#metadata-lookup-status')
			.filter({ hasText: 'Metadata applied to form.' })
			.waitFor();
		await ensureTitleValue(page, 'Dune');
		await page.click('[data-testid="metadata-lookup-close"]');
		await page.locator('[data-testid="metadata-lookup-modal"]').waitFor({ state: 'hidden' });
	});

	await runScenarioCheck(results, requireScenarioCheck(scenario, 'cover-art-load'), async () => {
		await page.fill('[data-testid="cover-art-url-input"]', 'https://example.com/dune-cover.jpg');
		await page.click('[data-testid="cover-art-url-load-btn"]');
		await page
			.locator('#cover-art-url-message')
			.filter({ hasText: 'Cover art loaded from URL.' })
			.waitFor();
		await page.locator('#cover-art-img:not(.hidden)').waitFor();
		await page.locator('#cover-art-img').evaluate((node) => {
			if (!(node instanceof HTMLImageElement)) {
				throw new Error('Expected cover-art image element.');
			}
			if (!node.src.startsWith('data:image/')) {
				throw new Error(`Expected cover-art preview data URL, received ${node.src}`);
			}
		});
	});

	return results;
}

export async function seedFileManagementScenario(
	page: Page,
	scenario: HarnessScenario,
): Promise<HarnessScenarioCheckResult[]> {
	await resetHarnessState(page);
	const results: HarnessScenarioCheckResult[] = [];

	await runScenarioCheck(
		results,
		requireScenarioCheck(scenario, 'selection-follows-reorder'),
		async () => {
			const items = page.locator('.file-list-item');
			await items.nth(1).click();
			await items.nth(1).locator('.move-up-btn').click();
			await page.locator('.context-filename').filter({ hasText: '02-dune-part-2.mp3' }).waitFor();
			await page.locator('.context-position').filter({ hasText: '1 of 2' }).waitFor();
		},
	);

	await runScenarioCheck(
		results,
		requireScenarioCheck(scenario, 'clear-and-reimport'),
		async () => {
			await page.locator('.inspector-context').filter({ hasText: '02-dune-part-2.mp3' }).waitFor();
			await page.click('#clear-files-btn');
			await page.locator('#file-count-display').filter({ hasText: '0 files' }).waitFor();
			await page.getByRole('button', { name: 'Add audio files' }).click();
			await page.locator('#file-count-display').filter({ hasText: '2 files' }).waitFor();
			await page.locator('.file-list-item').nth(1).waitFor();
			await page.locator('.inspector-context').filter({ hasText: 'No file selected' }).waitFor();
		},
	);

	return results;
}

export async function seedStatusScenario(
	page: Page,
	scenario: HarnessScenario,
): Promise<HarnessScenarioCheckResult[]> {
	await resetHarnessState(page);
	const results: HarnessScenarioCheckResult[] = [];
	await page.evaluate(async () => {
		await window.__ABB_HARNESS__?.seedOutput({
			outputDirectory: '/Library/Audiobooks',
			namingPreset: 'absDefault',
			absIncludeYear: false,
		});
	});
	await page.getByRole('button', { name: 'Process Audiobook' }).click();

	await runScenarioCheck(
		results,
		requireScenarioCheck(scenario, 'order-lock-visible'),
		async () => {
			await page.locator('#file-order-lock').waitFor();
			await page.locator('#clear-files-btn').evaluate((node) => {
				if (!(node instanceof HTMLButtonElement) || !node.disabled) {
					throw new Error('Expected clear-files button to be disabled while processing.');
				}
			});
		},
	);

	await runScenarioCheck(results, requireScenarioCheck(scenario, 'queue-completes'), async () => {
		await page.locator('#percentage-processed').filter({ hasText: '100.0%' }).waitFor();
		await page
			.locator('#job-list span')
			.filter({ hasText: 'Completed (100.0%)' })
			.first()
			.waitFor();
	});

	return results;
}

export async function seedOutputScenario(
	page: Page,
	scenario: HarnessScenario,
): Promise<HarnessScenarioCheckResult[]> {
	await resetHarnessState(page);
	await page.evaluate(async () => {
		await window.__ABB_HARNESS__?.seedMetadata({
			title: 'Dune',
			artist: 'Frank Herbert',
			series: 'Dune Chronicles',
			series_part: '1',
			date: '1965',
		});
		await window.__ABB_HARNESS__?.seedOutput({
			outputDirectory: '/Library/Audiobooks',
			namingPreset: 'absDefault',
			absIncludeYear: true,
		});
	});
	await page.locator('#output-preview-text').filter({ hasText: '/Library/Audiobooks' }).waitFor();
	const results: HarnessScenarioCheckResult[] = [];

	await runScenarioCheck(
		results,
		requireScenarioCheck(scenario, 'encoder-controls-reactive'),
		async () => {
			await page.selectOption('#adv-encoder', 'native_aac');
			await page.locator('#encoder-availability-hint').filter({ hasText: 'Native AAC' }).waitFor();
			await page.locator('#output-quality').evaluate((node) => {
				if (!node.classList.contains('hidden')) {
					throw new Error('Expected manual Native AAC selection to hide the quality select.');
				}
			});
		},
	);

	await runScenarioCheck(
		results,
		requireScenarioCheck(scenario, 'custom-template-row'),
		async () => {
			await page.selectOption('#output-naming-preset', 'customTemplate');
			await page.fill('#output-template-input', '{author}/{title}');
			await page.locator('#output-template-row').evaluate((node) => {
				if (node.hasAttribute('hidden')) {
					throw new Error('Expected custom template row to be visible.');
				}
			});
		},
	);

	await runScenarioCheck(
		results,
		requireScenarioCheck(scenario, 'preview-remains-anchored'),
		async () => {
			await page.locator('#output-preview-text').evaluate((node) => {
				const text = node.textContent ?? '';
				if (!text.includes('/Library/Audiobooks')) {
					throw new Error(
						`Expected preview text to stay anchored to /Library/Audiobooks, received ${text}`,
					);
				}
				if (text.includes('Select output directory')) {
					throw new Error('Expected preview text to resolve to a concrete path.');
				}
			});
		},
	);

	return results;
}

export async function seedHarnessScenario(
	page: Page,
	scenarioId: HarnessScenarioId,
	scenario: HarnessScenario,
): Promise<HarnessScenarioCheckResult[]> {
	switch (scenarioId) {
		case 'file-management':
			return seedFileManagementScenario(page, scenario);
		case 'metadata-edit':
			return seedMetadataScenario(page, scenario);
		case 'status-processing':
			return seedStatusScenario(page, scenario);
		case 'output-preview':
			return seedOutputScenario(page, scenario);
		default:
			throw new Error(`No seed runner implemented for scenario ${scenarioId}`);
	}
}
