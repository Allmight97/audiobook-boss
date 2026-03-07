#!/usr/bin/env bun

import { appendFile, mkdir, rm } from 'node:fs/promises';
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import path from 'node:path';

import { chromium, type Browser, type BrowserContext, type Page } from 'playwright';

import type {
	HarnessAgentSessionInfo,
	HarnessDomSummary,
	HarnessDomSummaryResult,
	HarnessOverflowCandidate,
	HarnessReviewFinding,
	HarnessReviewResult,
	HarnessViewportPreset,
	StartHarnessSessionOptions,
} from './api';
import { seedHarnessScenario } from './scenarioDriver';
import {
	gotoHarnessRoute,
	HARNESS_AGENT_ARTIFACT_ROOT,
	mirrorArtifactToLatest,
	startHarnessServer,
	summarizeConsoleMessage,
	writeJsonArtifact,
	type HarnessConsoleMessage,
} from './shared';
import {
	getHarnessScenario,
	type HarnessScenario,
	type HarnessScenarioId,
	type HarnessScenarioReviewAction,
	type HarnessScenarioReviewControl,
} from '../../src/harness/scenarios';

const SESSION_FILE = path.join(HARNESS_AGENT_ARTIFACT_ROOT, 'session.json');
const LATEST_ARTIFACT_DIR = path.join(HARNESS_AGENT_ARTIFACT_ROOT, 'latest');
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
		headed: payload?.headed ?? false,
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
	await writeJsonArtifact(SESSION_FILE, info);
	await writeJsonArtifact(path.join(LATEST_ARTIFACT_DIR, 'session.json'), info);
}

async function setViewport(page: Page, viewport: HarnessViewportPreset): Promise<void> {
	await page.setViewportSize(VIEWPORTS[viewport]);
}

async function configureSession(
	state: SessionState,
	options: Required<StartHarnessSessionOptions>,
): Promise<HarnessAgentSessionInfo> {
	resetRuntimeObservations(state);
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
		headed: options.headed,
	};
	await writeSessionInfo(state.info);
	return state.info;
}

function resetRuntimeObservations(state: SessionState): void {
	state.consoleMessages.length = 0;
	state.pageErrors.length = 0;
}

async function ensureScenarioLoaded(
	state: SessionState,
	scenarioId: HarnessScenarioId | undefined,
): Promise<HarnessScenario | null> {
	const resolvedScenarioId = scenarioId ?? state.info.scenario ?? undefined;
	if (!resolvedScenarioId) {
		return null;
	}
	const scenario = getHarnessScenario(resolvedScenarioId);
	if (state.info.scenario !== resolvedScenarioId) {
		resetRuntimeObservations(state);
		await gotoHarnessRoute(state.page, state.viteServer.origin, state.info.route);
		await seedHarnessScenario(state.page, resolvedScenarioId, scenario);
		state.info = {
			...state.info,
			scenario: resolvedScenarioId,
		};
		await writeSessionInfo(state.info);
	}
	return scenario;
}

async function captureSessionScreenshot(
	state: SessionState,
	label: string,
): Promise<{ screenshotPath: string; latestScreenshotPath: string }> {
	const filename = `${slugifyLabel(label)}.png`;
	const screenshotPath = path.join(state.info.artifactDir, filename);
	await state.page.screenshot({ path: screenshotPath, fullPage: true });
	const latestScreenshotPath = await mirrorArtifactToLatest(
		screenshotPath,
		path.join(LATEST_ARTIFACT_DIR, 'screenshot.png'),
	);
	return { screenshotPath, latestScreenshotPath };
}

async function captureDomSummary(
	state: SessionState,
	controls: readonly HarnessScenarioReviewControl[],
): Promise<HarnessDomSummaryResult> {
	const summary = await buildDomSummary(state.page, controls);
	const artifactPath = path.join(state.info.artifactDir, 'dom-summary.json');
	await writeJsonArtifact(artifactPath, summary);
	const latestArtifactPath = await mirrorArtifactToLatest(
		artifactPath,
		path.join(LATEST_ARTIFACT_DIR, 'dom-summary.json'),
	);
	return {
		summary,
		artifactPath,
		latestArtifactPath,
	};
}

async function buildDomSummary(
	page: Page,
	controls: readonly HarnessScenarioReviewControl[],
): Promise<HarnessDomSummary> {
	return page.evaluate((scenarioControls) => {
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
			keyControls: scenarioControls.map((control) =>
				describeControl(control.selector, control.label),
			),
			overflowCandidates,
		};
	}, controls);
}

async function runInteractiveChecks(
	state: SessionState,
	scenario: HarnessScenario | null,
): Promise<HarnessReviewFinding[]> {
	const failures: HarnessReviewFinding[] = [];
	const actionTimeout = 4_000;
	if (!scenario) {
		return failures;
	}

	for (const action of scenario.review.actions) {
		try {
			await runReviewAction(state, action, actionTimeout);
		} catch (error) {
			failures.push({
				id: action.id,
				message: `${action.label} ${error instanceof Error ? error.message : String(error)}`,
				selector: getActionSelector(action),
			});
		}
	}

	return failures;
}

function getActionSelector(action: HarnessScenarioReviewAction): string {
	switch (action.type) {
		case 'dialog-toggle':
			return action.triggerSelector;
		case 'select-option':
		case 'toggle-checkbox':
		case 'assert-text':
			return action.selector;
	}
}

async function runReviewAction(
	state: SessionState,
	action: HarnessScenarioReviewAction,
	actionTimeout: number,
): Promise<void> {
	switch (action.type) {
		case 'dialog-toggle': {
			const dialog = state.page.locator(action.dialogSelector);
			if (await dialog.isVisible().catch(() => false)) {
				await state.page.locator(action.dismissSelector).click({ timeout: actionTimeout });
				await dialog.waitFor({ state: 'hidden', timeout: actionTimeout });
			}

			await state.page.locator(action.triggerSelector).click({ timeout: actionTimeout });
			await dialog.waitFor({ state: 'visible', timeout: actionTimeout });
			await state.page.locator(action.dismissSelector).click({ timeout: actionTimeout });
			await dialog.waitFor({ state: 'hidden', timeout: actionTimeout });
			return;
		}
		case 'select-option': {
			await state.page.locator(action.selector).selectOption(action.optionValue, {
				timeout: actionTimeout,
			});
			if (action.assertVisibleSelector) {
				await state.page.locator(action.assertVisibleSelector).evaluate((node) => {
					if (!(node instanceof HTMLElement)) {
						throw new Error('Target element is not an HTMLElement.');
					}
					if (node.hidden || node.hasAttribute('hidden')) {
						throw new Error('Expected element to be visible, but it remained hidden.');
					}
				});
			}
			if (action.resetValue) {
				await state.page.locator(action.selector).selectOption(action.resetValue, {
					timeout: actionTimeout,
				});
			}
			return;
		}
		case 'toggle-checkbox': {
			const checkbox = state.page.locator(action.selector);
			const before = await checkbox.isChecked();
			await checkbox.click({ timeout: actionTimeout });
			const after = await checkbox.isChecked();
			if (before === after) {
				throw new Error('Checkbox state did not change after click.');
			}
			await checkbox.click({ timeout: actionTimeout });
			return;
		}
		case 'assert-text': {
			await state.page
				.locator(action.selector)
				.waitFor({ state: 'visible', timeout: actionTimeout });
			await state.page.locator(action.selector).evaluate((node, expectedText) => {
				const text = node.textContent ?? '';
				if (!text.includes(expectedText)) {
					throw new Error(`Expected "${expectedText}" in "${text}".`);
				}
			}, action.expectedText);
			return;
		}
	}
}

async function buildReviewResult(
	state: SessionState,
	viewport: HarnessViewportPreset,
	scenarioId?: HarnessScenarioId,
): Promise<HarnessReviewResult> {
	await setViewport(state.page, viewport);
	const scenario = await ensureScenarioLoaded(state, scenarioId);
	const domSnapshot = await captureDomSummary(state, scenario?.review.controls ?? []);
	const domSummary = domSnapshot.summary;
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

	if (scenario) {
		for (const target of scenario.review.advisoryTargets ?? []) {
			const visible = await state.page
				.locator(target.selector)
				.evaluate((node) => {
					if (!(node instanceof HTMLElement)) return false;
					const rect = node.getBoundingClientRect();
					const style = getComputedStyle(node);
					return (
						rect.width > 0 &&
						rect.height > 0 &&
						style.visibility !== 'hidden' &&
						style.display !== 'none'
					);
				})
				.catch(() => false);
			if (visible) {
				advisoryFindings.push({
					id: `review-target-${target.selector}`,
					message: target.message,
					selector: target.selector,
				});
			}
		}
	}

	objectiveFailures.push(...(await runInteractiveChecks(state, scenario)));

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
	const reviewPath = path.join(state.info.artifactDir, `review-${viewport}.json`);
	const reviewResult: HarnessReviewResult = {
		viewport,
		screenshotPath: screenshot.screenshotPath,
		latestScreenshotPath: screenshot.latestScreenshotPath,
		reviewPath,
		latestReviewPath: path.join(LATEST_ARTIFACT_DIR, 'review.json'),
		domSummary,
		objectiveFailures,
		advisoryFindings,
	};

	await writeJsonArtifact(reviewPath, reviewResult);
	await mirrorArtifactToLatest(reviewPath, reviewResult.latestReviewPath);
	return reviewResult;
}

async function reportTextNote(
	state: SessionState,
	message: string,
): Promise<{ notePath: string; latestNotePath: string }> {
	await appendFile(state.notePath, `[${new Date().toISOString()}] ${message.trim()}\n`, 'utf8');
	const latestNotePath = await mirrorArtifactToLatest(
		state.notePath,
		path.join(LATEST_ARTIFACT_DIR, 'notes.log'),
	);
	return { notePath: state.notePath, latestNotePath };
}

async function initializeState(
	startOptions: Required<StartHarnessSessionOptions>,
): Promise<SessionState> {
	const runId = new Date().toISOString().replace(/:/g, '-');
	const artifactDir = path.join(HARNESS_AGENT_ARTIFACT_ROOT, runId);
	await mkdir(artifactDir, { recursive: true });

	const viteServer = await startHarnessServer();
	const browser = await chromium.launch({ headless: !startOptions.headed });
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
		headed: startOptions.headed,
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
				resetRuntimeObservations(state);
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
				const scenario = await ensureScenarioLoaded(state, undefined);
				sendJson(response, 200, await captureDomSummary(state, scenario?.review.controls ?? []));
				return;
			}

			if (request.method === 'POST' && url.pathname === '/report-text') {
				const payload = await readJsonBody<{ message: string }>(request);
				sendJson(response, 200, await reportTextNote(state, payload.message));
				return;
			}

			if (request.method === 'POST' && url.pathname === '/review') {
				const payload = await readJsonBody<{
					viewport?: HarnessViewportPreset;
					scenario?: HarnessScenarioId;
				}>(request);
				sendJson(
					response,
					200,
					await buildReviewResult(state, payload.viewport ?? state.info.viewport, payload.scenario),
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
