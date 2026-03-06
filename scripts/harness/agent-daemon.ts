#!/usr/bin/env bun

import { appendFile, mkdir, rm, writeFile } from 'node:fs/promises';
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import path from 'node:path';

import { chromium, type Browser, type BrowserContext, type Page } from 'playwright';

import type {
	HarnessAgentSessionInfo,
	HarnessDomSummary,
	HarnessReviewFinding,
	HarnessReviewResult,
	HarnessViewportPreset,
	StartHarnessSessionOptions,
} from './api';
import { seedHarnessScenario } from './scenarioDriver';
import {
	gotoHarnessRoute,
	HARNESS_AGENT_ARTIFACT_ROOT,
	startHarnessServer,
	summarizeConsoleMessage,
	type HarnessConsoleMessage,
} from './shared';
import { getHarnessScenario, type HarnessScenarioId } from '../../src/harness/scenarios';

const SESSION_FILE = path.join(HARNESS_AGENT_ARTIFACT_ROOT, 'session.json');
const DEFAULT_ROUTE = '/harness.html';

const VIEWPORTS: Record<HarnessViewportPreset, { width: number; height: number }> = {
	desktop: { width: 1440, height: 960 },
	mobile: { width: 390, height: 844 },
};

type SessionState = {
	browser: Browser;
	context: BrowserContext;
	page: Page;
	apiServer: ReturnType<typeof createServer>;
	viteServer: Awaited<ReturnType<typeof startHarnessServer>>;
	info: HarnessAgentSessionInfo;
	consoleMessages: HarnessConsoleMessage[];
	pageErrors: string[];
	notePath: string;
	ready: boolean;
};

function resolveStartOptions(
	payload?: StartHarnessSessionOptions,
): Required<StartHarnessSessionOptions> {
	return {
		route: payload?.route ?? DEFAULT_ROUTE,
		viewport: payload?.viewport ?? 'desktop',
		scenario: payload?.scenario,
	};
}

function slugifyLabel(value: string): string {
	const normalized = value
		.trim()
		.toLowerCase()
		.replace(/[^a-z0-9._-]+/g, '-');
	return normalized.length > 0 ? normalized : 'capture';
}

async function readJsonBody<T>(request: IncomingMessage): Promise<T> {
	const chunks: Uint8Array[] = [];
	for await (const chunk of request) {
		chunks.push(chunk);
	}
	if (chunks.length === 0) {
		return {} as T;
	}
	return JSON.parse(Buffer.concat(chunks).toString('utf8')) as T;
}

function sendJson(response: ServerResponse, statusCode: number, body: unknown): void {
	response.statusCode = statusCode;
	response.setHeader('content-type', 'application/json');
	response.end(`${JSON.stringify(body, null, 2)}\n`);
}

async function writeSessionInfo(info: HarnessAgentSessionInfo): Promise<void> {
	await mkdir(HARNESS_AGENT_ARTIFACT_ROOT, { recursive: true });
	await writeFile(SESSION_FILE, `${JSON.stringify(info, null, 2)}\n`, 'utf8');
}

async function setViewport(page: Page, viewport: HarnessViewportPreset): Promise<void> {
	await page.setViewportSize(VIEWPORTS[viewport]);
}

async function configureSession(
	state: SessionState,
	options: Required<StartHarnessSessionOptions>,
): Promise<HarnessAgentSessionInfo> {
	await setViewport(state.page, options.viewport);
	await gotoHarnessRoute(state.page, state.viteServer.origin, options.route);
	if (options.scenario) {
		await seedHarnessScenario(state.page, options.scenario, getHarnessScenario(options.scenario));
	}

	state.info = {
		...state.info,
		route: options.route,
		viewport: options.viewport,
		scenario: options.scenario ?? null,
	};
	await writeSessionInfo(state.info);
	return state.info;
}

async function captureSessionScreenshot(
	state: SessionState,
	label: string,
): Promise<{ screenshotPath: string }> {
	const filename = `${slugifyLabel(label)}.png`;
	const screenshotPath = path.join(state.info.artifactDir, filename);
	await state.page.screenshot({ path: screenshotPath, fullPage: true });
	return { screenshotPath };
}

async function buildDomSummary(page: Page): Promise<HarnessDomSummary> {
	return page.evaluate(() => {
		const describeControl = (selector: string, label: string) => {
			const element = document.querySelector<HTMLElement>(selector);
			if (!element) {
				return {
					selector,
					label,
					visible: false,
					disabled: true,
					checked: null,
					value: null,
				};
			}
			const rect = element.getBoundingClientRect();
			const visible =
				rect.width > 0 &&
				rect.height > 0 &&
				getComputedStyle(element).visibility !== 'hidden' &&
				getComputedStyle(element).display !== 'none';
			const inputLike = element as HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;
			return {
				selector,
				label,
				visible,
				disabled: 'disabled' in inputLike ? Boolean(inputLike.disabled) : false,
				checked:
					inputLike instanceof HTMLInputElement &&
					(inputLike.type === 'checkbox' || inputLike.type === 'radio')
						? inputLike.checked
						: null,
				value: 'value' in inputLike ? String(inputLike.value ?? '') : null,
			};
		};

		const visibleElements = Array.from(document.querySelectorAll<HTMLElement>('body *')).filter(
			(element) => {
				const rect = element.getBoundingClientRect();
				const style = getComputedStyle(element);
				return (
					rect.width > 0 &&
					rect.height > 0 &&
					style.visibility !== 'hidden' &&
					style.display !== 'none'
				);
			},
		);

		const overflowCandidates: HarnessOverflowCandidate[] = visibleElements
			.map((element) => ({
				element,
				scrollWidth: element.scrollWidth,
				clientWidth: element.clientWidth,
				scrollHeight: element.scrollHeight,
				clientHeight: element.clientHeight,
			}))
			.filter(({ element, scrollWidth, clientWidth, scrollHeight, clientHeight }) => {
				const tag = element.tagName.toLowerCase();
				if (tag === 'textarea' || tag === 'select' || tag === 'input') {
					return false;
				}
				return scrollWidth > clientWidth + 6 || scrollHeight > clientHeight + 6;
			})
			.slice(0, 8)
			.map(({ element, scrollWidth, clientWidth, scrollHeight, clientHeight }) => ({
				selector: element.id
					? `#${element.id}`
					: element.getAttribute('data-testid')
						? `[data-testid="${element.getAttribute('data-testid')}"]`
						: element.tagName.toLowerCase(),
				text: (element.textContent ?? '').trim().slice(0, 120),
				scrollWidth,
				clientWidth,
				scrollHeight,
				clientHeight,
			}));

		const main = document.querySelector('main');
		const mainRect = main?.getBoundingClientRect() ?? new DOMRect(0, 0, 0, 0);

		return {
			viewport: {
				width: window.innerWidth,
				height: window.innerHeight,
				scrollWidth: document.documentElement.scrollWidth,
				scrollHeight: document.documentElement.scrollHeight,
				devicePixelRatio: window.devicePixelRatio,
			},
			mainRegion: {
				width: mainRect.width,
				height: mainRect.height,
				widthRatio: window.innerWidth > 0 ? mainRect.width / window.innerWidth : 0,
				left: mainRect.left,
				right: mainRect.right,
			},
			counts: {
				buttons: document.querySelectorAll('button').length,
				inputs: document.querySelectorAll('input').length,
				selects: document.querySelectorAll('select').length,
				checkboxes: document.querySelectorAll('input[type="checkbox"]').length,
				radios: document.querySelectorAll('input[type="radio"]').length,
				textareas: document.querySelectorAll('textarea').length,
				dialogs: document.querySelectorAll('[role="dialog"]').length,
			},
			horizontalOverflow: document.documentElement.scrollWidth > window.innerWidth + 2,
			keyControls: [
				describeControl('[data-testid="metadata-lookup-btn"]', 'Metadata lookup button'),
				describeControl('#metadata-save-btn', 'Metadata save button'),
				describeControl('#output-naming-preset', 'Output naming preset'),
				describeControl('#output-abs-include-year', 'Output include year toggle'),
				describeControl('#process-button', 'Process audiobook button'),
			],
			overflowCandidates,
		};
	});
}

async function runInteractiveChecks(state: SessionState): Promise<HarnessReviewFinding[]> {
	const failures: HarnessReviewFinding[] = [];
	const actionTimeout = 4_000;
	const modal = state.page.locator('#metadata-lookup-modal');
	if (await modal.isVisible().catch(() => false)) {
		await state.page.locator('[data-testid="metadata-lookup-close"]').click({
			timeout: actionTimeout,
		});
		await modal.waitFor({ state: 'hidden', timeout: actionTimeout });
	}

	try {
		const lookupButton = state.page.locator('[data-testid="metadata-lookup-btn"]');
		await lookupButton.click({ timeout: actionTimeout });
		await state.page.locator('#metadata-lookup-modal').waitFor({ timeout: 4_000 });
		await state.page.locator('[data-testid="metadata-lookup-close"]').click({
			timeout: actionTimeout,
		});
		await state.page.locator('#metadata-lookup-modal').waitFor({ state: 'hidden', timeout: 4_000 });
	} catch (error) {
		failures.push({
			id: 'metadata-lookup-modal',
			message: `Metadata lookup modal did not open and close cleanly: ${error instanceof Error ? error.message : String(error)}`,
			selector: '[data-testid="metadata-lookup-btn"]',
		});
	}

	try {
		await state.page.locator('#output-naming-preset').selectOption('customTemplate', {
			timeout: actionTimeout,
		});
		await state.page.locator('#output-template-row').evaluate((node) => {
			if (node.hasAttribute('hidden')) {
				throw new Error('Template row remained hidden after selecting custom template.');
			}
		});
		await state.page.locator('#output-naming-preset').selectOption('absDefault', {
			timeout: actionTimeout,
		});
	} catch (error) {
		failures.push({
			id: 'output-template-toggle',
			message: `Output naming preset did not expose the custom template row: ${error instanceof Error ? error.message : String(error)}`,
			selector: '#output-naming-preset',
		});
	}

	try {
		const checkbox = state.page.locator('#output-abs-include-year');
		const before = await checkbox.isChecked();
		await checkbox.click({ timeout: actionTimeout });
		const after = await checkbox.isChecked();
		if (before === after) {
			throw new Error('Checkbox state did not change after click.');
		}
		await checkbox.click({ timeout: actionTimeout });
	} catch (error) {
		failures.push({
			id: 'include-year-toggle',
			message: `Output include-year checkbox did not toggle cleanly: ${error instanceof Error ? error.message : String(error)}`,
			selector: '#output-abs-include-year',
		});
	}

	return failures;
}

async function buildReviewResult(
	state: SessionState,
	viewport: HarnessViewportPreset,
): Promise<HarnessReviewResult> {
	await setViewport(state.page, viewport);
	const domSummary = await buildDomSummary(state.page);
	const objectiveFailures: HarnessReviewFinding[] = [];
	const advisoryFindings: HarnessReviewFinding[] = [];

	const errorConsoleMessages = state.consoleMessages.filter((message) => message.type === 'error');
	if (state.pageErrors.length > 0 || errorConsoleMessages.length > 0) {
		objectiveFailures.push(
			...state.pageErrors.map((message) => ({
				id: 'pageerror',
				message,
			})),
			...errorConsoleMessages.map((message) => ({
				id: 'console-error',
				message: message.text,
			})),
		);
	}

	if (domSummary.horizontalOverflow) {
		objectiveFailures.push({
			id: 'horizontal-overflow',
			message:
				'The current viewport has horizontal overflow, which suggests clipping or layout spill.',
		});
	}

	for (const control of domSummary.keyControls) {
		if (!control.visible) {
			objectiveFailures.push({
				id: `missing-${control.selector}`,
				message: `${control.label} is not visible in the current harness view.`,
				selector: control.selector,
			});
		}
	}

	for (const candidate of domSummary.overflowCandidates) {
		advisoryFindings.push({
			id: `overflow-${candidate.selector}`,
			message: `Inspect ${candidate.selector} for clipping or density issues; the current review saw content larger than its container.`,
			selector: candidate.selector,
		});
	}

	objectiveFailures.push(...(await runInteractiveChecks(state)));

	if (viewport === 'desktop' && domSummary.mainRegion.widthRatio < 0.58) {
		advisoryFindings.push({
			id: 'desktop-gutter-balance',
			message:
				'The main content lane uses less than 58% of the desktop viewport width; review for wasted horizontal space.',
			selector: 'main',
		});
	}

	const screenshot = await captureSessionScreenshot(
		state,
		`review-${viewport}-${new Date().toISOString().replace(/:/g, '-')}`,
	);
	const reviewResult: HarnessReviewResult = {
		viewport,
		screenshotPath: screenshot.screenshotPath,
		domSummary,
		objectiveFailures,
		advisoryFindings,
	};

	await writeFile(
		path.join(state.info.artifactDir, `review-${viewport}.json`),
		`${JSON.stringify(reviewResult, null, 2)}\n`,
		'utf8',
	);
	return reviewResult;
}

async function reportTextNote(state: SessionState, message: string): Promise<{ notePath: string }> {
	await appendFile(state.notePath, `[${new Date().toISOString()}] ${message.trim()}\n`, 'utf8');
	return { notePath: state.notePath };
}

async function initializeState(
	startOptions: Required<StartHarnessSessionOptions>,
): Promise<SessionState> {
	const runId = new Date().toISOString().replace(/:/g, '-');
	const artifactDir = path.join(HARNESS_AGENT_ARTIFACT_ROOT, runId);
	await mkdir(artifactDir, { recursive: true });

	const viteServer = await startHarnessServer();
	const browser = await chromium.launch({ headless: true });
	const context = await browser.newContext({
		viewport: VIEWPORTS[startOptions.viewport],
	});
	const page = await context.newPage();
	const info: HarnessAgentSessionInfo = {
		sessionId: `harness-agent-${runId}`,
		pid: process.pid,
		port: 0,
		artifactDir,
		route: startOptions.route,
		viewport: startOptions.viewport,
		scenario: startOptions.scenario ?? null,
		startedAt: new Date().toISOString(),
	};

	const state: SessionState = {
		browser,
		context,
		page,
		apiServer: createServer(),
		viteServer,
		info,
		consoleMessages: [],
		pageErrors: [],
		notePath: path.join(artifactDir, 'notes.log'),
		ready: false,
	};

	page.on('console', (message) => {
		state.consoleMessages.push(summarizeConsoleMessage(message));
	});
	page.on('pageerror', (error) => {
		state.pageErrors.push(error.message);
	});

	return state;
}

async function closeState(state: SessionState): Promise<void> {
	await state.context.close();
	await state.browser.close();
	await state.viteServer.server.close();
	await new Promise<void>((resolve, reject) => {
		state.apiServer.close((error) => {
			if (error) {
				reject(error);
				return;
			}
			resolve();
		});
	});
	await rm(SESSION_FILE, { force: true });
}

async function main(): Promise<void> {
	const rawOptions = process.argv[2];
	const startOptions = resolveStartOptions(rawOptions ? JSON.parse(rawOptions) : undefined);
	const state = await initializeState(startOptions);

	state.apiServer = createServer(async (request, response) => {
		try {
			const url = new URL(request.url ?? '/', 'http://127.0.0.1');
			if (request.method === 'GET' && url.pathname === '/health') {
				if (!state.ready) {
					sendJson(response, 503, { error: 'Harness agent session is still initializing.' });
					return;
				}
				sendJson(response, 200, state.info);
				return;
			}

			if (request.method === 'POST' && url.pathname === '/configure') {
				const payload = await readJsonBody<StartHarnessSessionOptions>(request);
				sendJson(response, 200, await configureSession(state, resolveStartOptions(payload)));
				return;
			}

			if (request.method === 'POST' && url.pathname === '/seed') {
				const payload = await readJsonBody<{ scenario: HarnessScenarioId }>(request);
				const scenario = getHarnessScenario(payload.scenario);
				await gotoHarnessRoute(state.page, state.viteServer.origin, state.info.route);
				await seedHarnessScenario(state.page, payload.scenario, scenario);
				state.info = {
					...state.info,
					scenario: payload.scenario,
				};
				await writeSessionInfo(state.info);
				sendJson(response, 200, state.info);
				return;
			}

			if (request.method === 'POST' && url.pathname === '/screenshot') {
				const payload = await readJsonBody<{ label: string }>(request);
				sendJson(response, 200, await captureSessionScreenshot(state, payload.label));
				return;
			}

			if (request.method === 'GET' && url.pathname === '/dom-summary') {
				sendJson(response, 200, await buildDomSummary(state.page));
				return;
			}

			if (request.method === 'POST' && url.pathname === '/report-text') {
				const payload = await readJsonBody<{ message: string }>(request);
				sendJson(response, 200, await reportTextNote(state, payload.message));
				return;
			}

			if (request.method === 'POST' && url.pathname === '/review') {
				const payload = await readJsonBody<{ viewport?: HarnessViewportPreset }>(request);
				sendJson(
					response,
					200,
					await buildReviewResult(state, payload.viewport ?? state.info.viewport),
				);
				return;
			}

			if (request.method === 'POST' && url.pathname === '/close') {
				sendJson(response, 200, { closed: true });
				setTimeout(() => {
					void closeState(state).finally(() => process.exit(0));
				}, 50);
				return;
			}

			sendJson(response, 404, { error: 'Not found' });
		} catch (error) {
			sendJson(response, 500, {
				error: error instanceof Error ? error.message : String(error),
			});
		}
	});

	await new Promise<void>((resolve) => {
		state.apiServer.listen(0, '127.0.0.1', () => {
			resolve();
		});
	});

	const address = state.apiServer.address();
	if (!address || typeof address === 'string') {
		throw new Error('Harness agent daemon failed to bind a local control port.');
	}

	state.info = {
		...state.info,
		port: address.port,
	};
	await writeSessionInfo(state.info);
	await configureSession(state, startOptions);
	state.ready = true;
}

main().catch(async (error) => {
	console.error(`[harness:agent-daemon] ${error instanceof Error ? error.message : String(error)}`);
	await rm(SESSION_FILE, { force: true });
	process.exit(1);
});
