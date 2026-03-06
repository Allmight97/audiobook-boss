#!/usr/bin/env bun

import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { chromium, type ConsoleMessage, type Page } from 'playwright';
import { createServer, type ViteDevServer } from 'vite';

import {
	findUnmappedHarnessUiPaths,
	getHarnessScenario,
	resolveHarnessScenariosForPaths,
	type HarnessScenario,
	type HarnessScenarioId,
} from '../../src/harness/scenarios.ts';

type CliOptions =
	| {
			mode: 'scenario';
			scenarioIds: HarnessScenarioId[];
	  }
	| {
			mode: 'changed';
	  };

type ScenarioArtifactSummary = {
	id: HarnessScenarioId;
	title: string;
	screenshotPath: string;
	consoleMessages: Array<{ type: string; text: string }>;
	pageErrors: string[];
};

const ARTIFACT_ROOT = path.resolve('.artifacts/harness');
const SERVER_HOST = '127.0.0.1';
const SERVER_PORT = 4173;

function parseArgs(argv: string[]): CliOptions {
	if (argv.length === 0 || argv.includes('--changed')) {
		return { mode: 'changed' };
	}

	const scenarioIds: HarnessScenarioId[] = [];
	for (let index = 0; index < argv.length; index += 1) {
		const arg = argv[index];
		if (arg !== '--scenario') {
			throw new Error(`Unknown argument: ${arg}`);
		}
		const next = argv[index + 1];
		if (!next) {
			throw new Error('Missing value for --scenario');
		}
		scenarioIds.push(next as HarnessScenarioId);
		index += 1;
	}

	if (scenarioIds.length === 0) {
		throw new Error('Pass --changed or at least one --scenario <id>');
	}

	return { mode: 'scenario', scenarioIds };
}

function runGit(args: string[]): string {
	const result = spawnSync('git', args, {
		cwd: process.cwd(),
		encoding: 'utf8',
	});
	if (result.status !== 0) {
		throw new Error(result.stderr.trim() || `git ${args.join(' ')} failed`);
	}
	return result.stdout.trim();
}

function tryGit(args: string[]): string {
	const result = spawnSync('git', args, {
		cwd: process.cwd(),
		encoding: 'utf8',
	});
	if (result.status !== 0) {
		return '';
	}
	return result.stdout.trim();
}

function getMergeBase(): string | null {
	const mainRef = tryGit(['rev-parse', '--verify', 'main']);
	if (!mainRef) return null;
	const mergeBase = tryGit(['merge-base', 'HEAD', 'main']);
	return mergeBase || null;
}

function listChangedFiles(): string[] {
	const candidates = new Set<string>();
	const mergeBase = getMergeBase();

	if (mergeBase) {
		const branchDiff = runGit(['diff', '--name-only', `${mergeBase}...HEAD`]);
		for (const line of branchDiff.split('\n')) {
			if (line) candidates.add(line);
		}
	}

	const workingTreeDiff = tryGit(['diff', '--name-only', 'HEAD', '--']);
	for (const line of workingTreeDiff.split('\n')) {
		if (line) candidates.add(line);
	}

	const untracked = tryGit(['ls-files', '--others', '--exclude-standard']);
	for (const line of untracked.split('\n')) {
		if (line) candidates.add(line);
	}

	return [...candidates];
}

function resolveScenarios(options: CliOptions): HarnessScenario[] {
	if (options.mode === 'scenario') {
		return options.scenarioIds.map((id) => getHarnessScenario(id));
	}

	const changedFiles = listChangedFiles();
	if (changedFiles.length === 0) {
		console.log('[harness:verify] No branch or working-tree changes detected; skipping.');
		return [];
	}

	const unmappedUiPaths = findUnmappedHarnessUiPaths(changedFiles);
	if (unmappedUiPaths.length > 0) {
		throw new Error(
			[
				'Harness verification found UI-affecting paths with no scenario coverage.',
				'Add or extend a scenario in src/harness/scenarios.ts before claiming the UI work is done.',
				...unmappedUiPaths.map((entry) => `- ${entry}`),
			].join('\n'),
		);
	}

	const scenarios = resolveHarnessScenariosForPaths(changedFiles);
	if (scenarios.length === 0) {
		console.log('[harness:verify] No harness-covered UI changes detected; skipping.');
		return [];
	}

	console.log(
		`[harness:verify] Resolved scenarios from changed files: ${scenarios.map((scenario) => scenario.id).join(', ')}`,
	);
	return scenarios;
}

async function startServer(): Promise<ViteDevServer> {
	const server = await createServer({
		server: {
			host: SERVER_HOST,
			port: SERVER_PORT,
			strictPort: true,
		},
	});
	await server.listen();
	return server;
}

async function gotoHarness(page: Page, scenario: HarnessScenario): Promise<void> {
	await page.goto(`http://${SERVER_HOST}:${SERVER_PORT}${scenario.route}`, {
		waitUntil: 'load',
	});
	await page.waitForFunction(() => typeof window.__ABB_HARNESS__ !== 'undefined', {
		timeout: 10_000,
	});
	await page.evaluate(async () => {
		await window.__ABB_HARNESS__?.reset();
	});
}

async function runMetadataScenario(page: Page, scenario: HarnessScenario): Promise<void> {
	await gotoHarness(page, scenario);

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
	await page.locator('#meta-title').evaluate((node) => {
		if (!(node instanceof HTMLInputElement)) {
			throw new Error('Expected metadata title input to be an HTMLInputElement.');
		}
		if (node.value !== 'Dune') {
			throw new Error(`Expected metadata title to be Dune, received ${node.value}`);
		}
	});
}

async function runStatusScenario(page: Page, scenario: HarnessScenario): Promise<void> {
	await gotoHarness(page, scenario);

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

async function runOutputScenario(page: Page, scenario: HarnessScenario): Promise<void> {
	await gotoHarness(page, scenario);

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

async function runScenario(page: Page, scenario: HarnessScenario): Promise<void> {
	switch (scenario.id) {
		case 'metadata-edit':
			await runMetadataScenario(page, scenario);
			return;
		case 'status-processing':
			await runStatusScenario(page, scenario);
			return;
		case 'output-preview':
			await runOutputScenario(page, scenario);
			return;
		default:
			throw new Error(`No runner implemented for scenario ${scenario.id}`);
	}
}

function summarizeConsoleMessage(message: ConsoleMessage): { type: string; text: string } {
	return {
		type: message.type(),
		text: message.text(),
	};
}

async function writeScenarioSummary(
	artifactDir: string,
	summary: ScenarioArtifactSummary,
): Promise<void> {
	await writeFile(
		path.join(artifactDir, 'summary.json'),
		`${JSON.stringify(summary, null, 2)}\n`,
		'utf8',
	);
}

async function main(): Promise<void> {
	const options = parseArgs(process.argv.slice(2));
	const scenarios = resolveScenarios(options);
	if (scenarios.length === 0) {
		return;
	}

	if (options.mode === 'changed' && scenarios.length > 1) {
		for (const scenario of scenarios) {
			const result = spawnSync(
				process.execPath,
				[path.resolve('scripts/harness/verify.ts'), '--scenario', scenario.id],
				{
					cwd: process.cwd(),
					encoding: 'utf8',
				},
			);
			if (result.stdout) process.stdout.write(result.stdout);
			if (result.stderr) process.stderr.write(result.stderr);
			if (result.status !== 0) {
				throw new Error(`Scenario ${scenario.id} failed during --changed verification.`);
			}
		}
		return;
	}

	await mkdir(ARTIFACT_ROOT, { recursive: true });
	const runId = new Date().toISOString().replaceAll(':', '-');
	const runArtifactDir = path.join(ARTIFACT_ROOT, runId);
	await mkdir(runArtifactDir, { recursive: true });

	const server = await startServer();
	try {
		for (const scenario of scenarios) {
			const artifactDir = path.join(runArtifactDir, scenario.id);
			await mkdir(artifactDir, { recursive: true });

			const browser = await chromium.launch({ headless: true });
			try {
				const page = await browser.newPage();
				const consoleMessages: ScenarioArtifactSummary['consoleMessages'] = [];
				const pageErrors: string[] = [];

				page.on('console', (message) => {
					consoleMessages.push(summarizeConsoleMessage(message));
				});
				page.on('pageerror', (error) => {
					pageErrors.push(error.message);
				});

				try {
					await runScenario(page, scenario);
				} finally {
					const screenshotPath = path.join(artifactDir, scenario.screenshotName);
					await page.screenshot({ path: screenshotPath, fullPage: true });
					const summary: ScenarioArtifactSummary = {
						id: scenario.id,
						title: scenario.title,
						screenshotPath,
						consoleMessages,
						pageErrors,
					};
					await writeScenarioSummary(artifactDir, summary);
					await page.close();
				}

				const errorConsoleMessages = consoleMessages.filter((message) => message.type === 'error');
				if (pageErrors.length > 0 || errorConsoleMessages.length > 0) {
					throw new Error(
						[
							`Harness scenario ${scenario.id} emitted runtime errors.`,
							...pageErrors.map((entry) => `pageerror: ${entry}`),
							...errorConsoleMessages.map((entry) => `console.error: ${entry.text}`),
							`Artifacts: ${artifactDir}`,
						].join('\n'),
					);
				}

				console.log(`[harness:verify] ${scenario.id} passed. Artifacts: ${artifactDir}`);
			} finally {
				await browser.close();
			}
		}
	} finally {
		await server.close();
	}
}

main()
	.then(() => {
		process.exit(0);
	})
	.catch((error) => {
		console.error(`[harness:verify] ${error instanceof Error ? error.message : String(error)}`);
		process.exit(1);
	});
