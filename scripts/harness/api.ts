import { mkdir, readFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { spawn } from 'node:child_process';

import type { HarnessScenarioId } from '../../src/harness/scenarios';
import { HARNESS_AGENT_ARTIFACT_ROOT } from './shared';

export type HarnessViewportPreset = 'desktop' | 'mobile';

export type StartHarnessSessionOptions = {
	scenario?: HarnessScenarioId;
	route?: string;
	viewport?: HarnessViewportPreset;
};

export type HarnessControlSummary = {
	selector: string;
	label: string;
	visible: boolean;
	disabled: boolean;
	checked: boolean | null;
	value: string | null;
};

export type HarnessOverflowCandidate = {
	selector: string;
	text: string;
	scrollWidth: number;
	clientWidth: number;
	scrollHeight: number;
	clientHeight: number;
};

export type HarnessDomSummary = {
	viewport: {
		width: number;
		height: number;
		scrollWidth: number;
		scrollHeight: number;
		devicePixelRatio: number;
	};
	mainRegion: {
		width: number;
		height: number;
		widthRatio: number;
		left: number;
		right: number;
	};
	counts: {
		buttons: number;
		inputs: number;
		selects: number;
		checkboxes: number;
		radios: number;
		textareas: number;
		dialogs: number;
	};
	horizontalOverflow: boolean;
	keyControls: HarnessControlSummary[];
	overflowCandidates: HarnessOverflowCandidate[];
};

export type HarnessReviewFinding = {
	id: string;
	message: string;
	selector?: string;
};

export type HarnessReviewResult = {
	viewport: HarnessViewportPreset;
	screenshotPath: string;
	domSummary: HarnessDomSummary;
	objectiveFailures: HarnessReviewFinding[];
	advisoryFindings: HarnessReviewFinding[];
};

export type HarnessAgentSessionInfo = {
	sessionId: string;
	pid: number;
	port: number;
	artifactDir: string;
	route: string;
	viewport: HarnessViewportPreset;
	scenario: HarnessScenarioId | null;
	startedAt: string;
};

const SESSION_FILE = path.join(HARNESS_AGENT_ARTIFACT_ROOT, 'session.json');
const DEFAULT_ROUTE = '/harness.html';
const DEFAULT_VIEWPORT: HarnessViewportPreset = 'desktop';
const HARNESS_RUNTIME_EXECUTABLE = process.versions.bun ? process.execPath : 'bun';

type SessionRequest<TBody> = {
	method?: 'GET' | 'POST';
	path: string;
	body?: TBody;
};

function getDefaultOptions(
	options: StartHarnessSessionOptions = {},
): Required<StartHarnessSessionOptions> {
	return {
		route: options.route ?? DEFAULT_ROUTE,
		viewport: options.viewport ?? DEFAULT_VIEWPORT,
		scenario: options.scenario,
	};
}

export async function readHarnessSessionInfo(): Promise<HarnessAgentSessionInfo | null> {
	try {
		const raw = await readFile(SESSION_FILE, 'utf8');
		return JSON.parse(raw) as HarnessAgentSessionInfo;
	} catch {
		return null;
	}
}

async function requestSession<TResponse, TBody = undefined>(
	session: HarnessAgentSessionInfo,
	request: SessionRequest<TBody>,
): Promise<TResponse> {
	const response = await fetch(`http://127.0.0.1:${session.port}${request.path}`, {
		method: request.method ?? 'GET',
		headers: request.body ? { 'content-type': 'application/json' } : undefined,
		body: request.body ? JSON.stringify(request.body) : undefined,
	});
	if (!response.ok) {
		const detail = await response.text();
		throw new Error(
			`Harness agent request failed: ${response.status} ${response.statusText}${detail ? ` :: ${detail.trim()}` : ''}`,
		);
	}
	return (await response.json()) as TResponse;
}

function isLivePid(pid: number): boolean {
	try {
		process.kill(pid, 0);
		return true;
	} catch {
		return false;
	}
}

async function waitForSessionReady(timeoutMs = 30_000): Promise<HarnessAgentSessionInfo> {
	const start = Date.now();
	while (Date.now() - start < timeoutMs) {
		const session = await readHarnessSessionInfo();
		if (session && isLivePid(session.pid)) {
			try {
				await requestSession<HarnessAgentSessionInfo, undefined>(session, { path: '/health' });
				return session;
			} catch {
				// Keep polling until the daemon is ready.
			}
		}
		await new Promise((resolve) => setTimeout(resolve, 150));
	}
	throw new Error('Timed out waiting for harness agent session to become ready.');
}

async function waitForSessionClosed(pid: number, timeoutMs = 10_000): Promise<void> {
	const start = Date.now();
	while (Date.now() - start < timeoutMs) {
		const session = await readHarnessSessionInfo();
		if (!session && !isLivePid(pid)) {
			return;
		}
		await new Promise((resolve) => setTimeout(resolve, 150));
	}
	throw new Error('Timed out waiting for harness agent session to close.');
}

async function spawnHarnessDaemon(options: Required<StartHarnessSessionOptions>): Promise<void> {
	await mkdir(HARNESS_AGENT_ARTIFACT_ROOT, { recursive: true });
	const child = spawn(
		HARNESS_RUNTIME_EXECUTABLE,
		[path.resolve('scripts/harness/agent-daemon.ts'), JSON.stringify(options)],
		{
			cwd: process.cwd(),
			detached: true,
			stdio: 'ignore',
		},
	);
	child.unref();
}

export async function startHarnessSession(
	options: StartHarnessSessionOptions = {},
): Promise<HarnessAgentSessionInfo> {
	const resolved = getDefaultOptions(options);
	const existing = await readHarnessSessionInfo();
	if (existing && isLivePid(existing.pid)) {
		try {
			return await requestSession<HarnessAgentSessionInfo, Required<StartHarnessSessionOptions>>(
				existing,
				{
					method: 'POST',
					path: '/configure',
					body: resolved,
				},
			);
		} catch {
			try {
				process.kill(existing.pid, 'SIGTERM');
			} catch {
				// Best effort cleanup for a stale harness agent process.
			}
			await rm(SESSION_FILE, { force: true });
		}
	}

	if (existing && !isLivePid(existing.pid)) {
		await rm(SESSION_FILE, { force: true });
	}

	await spawnHarnessDaemon(resolved);
	return waitForSessionReady();
}

async function requireLiveSession(): Promise<HarnessAgentSessionInfo> {
	const session = await readHarnessSessionInfo();
	if (!session || !isLivePid(session.pid)) {
		throw new Error('No live harness agent session found. Start one with `bun run harness:agent`.');
	}
	return session;
}

export async function seedScenario(scenario: HarnessScenarioId): Promise<HarnessAgentSessionInfo> {
	const session = await requireLiveSession();
	return requestSession<HarnessAgentSessionInfo, { scenario: HarnessScenarioId }>(session, {
		method: 'POST',
		path: '/seed',
		body: { scenario },
	});
}

export async function captureScreenshot(label: string): Promise<{ screenshotPath: string }> {
	const session = await requireLiveSession();
	return requestSession<{ screenshotPath: string }, { label: string }>(session, {
		method: 'POST',
		path: '/screenshot',
		body: { label },
	});
}

export async function getDomSummary(): Promise<HarnessDomSummary> {
	const session = await requireLiveSession();
	return requestSession<HarnessDomSummary, undefined>(session, {
		path: '/dom-summary',
	});
}

export async function reportText(message: string): Promise<{ notePath: string }> {
	const session = await requireLiveSession();
	return requestSession<{ notePath: string }, { message: string }>(session, {
		method: 'POST',
		path: '/report-text',
		body: { message },
	});
}

export async function runUiReviewChecklist(
	options: { viewport?: HarnessViewportPreset } = {},
): Promise<HarnessReviewResult> {
	const session = await requireLiveSession();
	return requestSession<HarnessReviewResult, { viewport?: HarnessViewportPreset }>(session, {
		method: 'POST',
		path: '/review',
		body: options,
	});
}

export async function closeHarnessSession(): Promise<void> {
	const session = await readHarnessSessionInfo();
	if (!session) return;

	if (isLivePid(session.pid)) {
		try {
			await requestSession<{ closed: boolean }, undefined>(session, {
				method: 'POST',
				path: '/close',
			});
		} catch {
			process.kill(session.pid, 'SIGTERM');
		}
	}

	await waitForSessionClosed(session.pid).catch(async () => {
		await rm(SESSION_FILE, { force: true });
	});
}
