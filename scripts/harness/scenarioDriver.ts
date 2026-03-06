import type { Page } from 'playwright';

import { resetHarnessState } from './shared';
import type { HarnessScenario, HarnessScenarioId } from '../../src/harness/scenarios';

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

export async function seedMetadataScenario(page: Page, _scenario: HarnessScenario): Promise<void> {
	await resetHarnessState(page);
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
}

export async function seedStatusScenario(page: Page, _scenario: HarnessScenario): Promise<void> {
	await resetHarnessState(page);
	await page.evaluate(async () => {
		await window.__ABB_HARNESS__?.seedOutput({
			outputDirectory: '/Library/Audiobooks',
			namingPreset: 'absDefault',
			absIncludeYear: false,
		});
	});
	await page.getByRole('button', { name: 'Process Audiobook' }).click();
	await page.locator('#percentage-processed').filter({ hasText: '100.0%' }).waitFor();
	await page.getByText('Completed (100.0%)').waitFor();
}

export async function seedOutputScenario(page: Page, _scenario: HarnessScenario): Promise<void> {
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

	await page.selectOption('#output-naming-preset', 'customTemplate');
	await page.fill('#output-template-input', '{author}/{title}');
	await page.locator('#output-template-row').evaluate((node) => {
		if (node.hasAttribute('hidden')) {
			throw new Error('Expected custom template row to be visible.');
		}
	});
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
}

export async function seedHarnessScenario(
	page: Page,
	scenarioId: HarnessScenarioId,
	scenario: HarnessScenario,
): Promise<void> {
	switch (scenarioId) {
		case 'metadata-edit':
			await seedMetadataScenario(page, scenario);
			return;
		case 'status-processing':
			await seedStatusScenario(page, scenario);
			return;
		case 'output-preview':
			await seedOutputScenario(page, scenario);
			return;
		default:
			throw new Error(`No seed runner implemented for scenario ${scenarioId}`);
	}
}
