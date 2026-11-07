import type { FrontendLogLevel } from './generated/tauri';
import { isAppErrorEnvelope } from './tauri/appError';
import { tauriClient } from './tauri/client';

const MAX_EVENTS_PER_WINDOW = 20;
const RATE_LIMIT_WINDOW_MS = 60_000;

let installed = false;
let windowStartMs = 0;
let sentInWindow = 0;
let droppedInWindow = 0;

function summarizeRejectionReason(reason: unknown): { name: string; message: string } {
	if (reason instanceof Error) {
		return { name: reason.name || 'Error', message: reason.message || 'Unknown error' };
	}
	if (isAppErrorEnvelope(reason)) {
		return { name: reason.category, message: reason.message };
	}
	if (typeof reason === 'string') {
		return { name: 'Rejection', message: `String rejection value (${reason.length} chars)` };
	}
	return { name: 'Rejection', message: 'Non-error rejection value' };
}

function summarizeErrorEvent(event: ErrorEvent): { name: string; message: string } {
	if (event.error instanceof Error) {
		return {
			name: event.error.name || 'Error',
			message: event.error.message || event.message || 'Unknown error',
		};
	}
	return { name: 'Error', message: event.message || 'Unknown error' };
}

function rollRateLimitWindow(nowMs: number): number {
	if (nowMs - windowStartMs < RATE_LIMIT_WINDOW_MS) return 0;
	const droppedSinceLastReset = droppedInWindow;
	windowStartMs = nowMs;
	sentInWindow = 0;
	droppedInWindow = 0;
	return droppedSinceLastReset;
}

function sendLogFrontend(level: FrontendLogLevel, scope: string, message: string): void {
	void tauriClient.logFrontend({ level, scope, message }).catch(() => undefined);
}

function forward(scope: string, message: string): void {
	const droppedSinceLastReset = rollRateLimitWindow(Date.now());
	if (droppedSinceLastReset > 0) {
		sendLogFrontend(
			'warn',
			'frontendLogBridge.rateLimit',
			`Dropped ${droppedSinceLastReset} frontend log event(s) over the ${MAX_EVENTS_PER_WINDOW}/60s fixed-window limit.`,
		);
	}
	if (sentInWindow >= MAX_EVENTS_PER_WINDOW) {
		droppedInWindow += 1;
		return;
	}
	sentInWindow += 1;
	sendLogFrontend('error', scope, message);
}

function summarizeFailure(cause: unknown): string {
	if (isAppErrorEnvelope(cause)) return `code=${cause.code} category=${cause.category}`;
	const { name, message } = summarizeRejectionReason(cause);
	return `${name}: ${message}`;
}

export function logFrontendFailure(scope: string, cause: unknown): void {
	if (!installed) return;
	forward(scope, summarizeFailure(cause));
}

function handleWindowError(event: ErrorEvent): void {
	const { name, message } = summarizeErrorEvent(event);
	forward(`window.error:${name}`, message);
}

function handleUnhandledRejection(event: PromiseRejectionEvent): void {
	const { name, message } = summarizeRejectionReason(event.reason);
	forward(`window.unhandledrejection:${name}`, message);
}

export function initFrontendErrorLogBridge(): void {
	if (installed) return;
	if (!(window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__) return;
	installed = true;
	window.addEventListener('error', handleWindowError);
	window.addEventListener('unhandledrejection', handleUnhandledRejection);
}
