import { tauriClient } from './tauri/client';
import { isAppErrorEnvelope } from './tauri/appError';
import type { FrontendLogLevel } from '../types/frontendLog';

/**
 * Forwards webview `error`/`unhandledrejection` events into the captured
 * backend dev logs (`.logs/tauri-dev.log` only tees Rust stdout/stderr, so
 * frontend failures were otherwise invisible there — see docs/DECISIONS.md
 * "Frontend failures enter dev logs through one sanitized bridge").
 *
 * Sanitization is deliberate and narrow: only an error name/category plus a
 * truncated message string ever leave this module. Never forward arbitrary
 * console objects or string-rejection bodies. Short Error/AppError messages
 * are kept for local diagnosis (paths may appear); provider payloads and secrets
 * must not be logged deliberately.
 */
const MESSAGE_MAX_CHARS = 500;
const SCOPE_MAX_CHARS = 200;
const MAX_EVENTS_PER_WINDOW = 20;
const RATE_LIMIT_WINDOW_MS = 60_000;

let installed = false;
let windowStartMs = 0;
let sentInWindow = 0;
let droppedInWindow = 0;

/** Flattens log-breaking control characters before IPC. Exported for tests. */
export function flattenLogText(value: string): string {
	return value
		.replace(/[\r\n\t]+/g, ' ')
		.replace(/[\u0000-\u0008\u000B-\u001F\u007F-\u009F]/g, '');
}

function truncate(value: string, maxChars: number): string {
	if (value.length <= maxChars) return value;
	return `${value.slice(0, maxChars)}…`;
}

function sanitizeOutboundField(value: string, maxChars: number): string {
	return truncate(flattenLogText(value), maxChars);
}

function summarizeRejectionReason(reason: unknown): { name: string; message: string } {
	if (reason instanceof Error) {
		return { name: reason.name || 'Error', message: reason.message || 'Unknown error' };
	}
	if (isAppErrorEnvelope(reason)) {
		return { name: reason.category, message: reason.message };
	}
	// String reasons are arbitrary caller payloads — forwarding them verbatim
	// would bypass the sanitization guardrail (they can carry paths/secrets).
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

/** Resets the fixed rate-limit window and reports how many events the
 * previous window dropped, if any. */
function rollRateLimitWindow(nowMs: number): number {
	if (nowMs - windowStartMs < RATE_LIMIT_WINDOW_MS) {
		return 0;
	}
	const droppedSinceLastReset = droppedInWindow;
	windowStartMs = nowMs;
	sentInWindow = 0;
	droppedInWindow = 0;
	return droppedSinceLastReset;
}

function sendLogFrontend(level: FrontendLogLevel, scope: string, message: string): void {
	// Synchronous try: outside a Tauri webview (plain Vite dev, lab.html) the
	// generated invoke reads window.__TAURI_INTERNALS__ synchronously and
	// throws before any promise exists — an error handler must never itself
	// throw a second error per event.
	try {
		void tauriClient
			.logFrontend({
				level,
				scope: sanitizeOutboundField(scope, SCOPE_MAX_CHARS),
				message: sanitizeOutboundField(message, MESSAGE_MAX_CHARS),
			})
			.catch(() => undefined);
	} catch {
		// Not running under Tauri; the console still has the original error.
	}
}

function forward(scope: string, message: string): void {
	const nowMs = Date.now();
	const droppedSinceLastReset = rollRateLimitWindow(nowMs);
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

function handleWindowError(event: ErrorEvent): void {
	const { name, message } = summarizeErrorEvent(event);
	forward(`window.error:${name}`, message);
}

function handleUnhandledRejection(event: PromiseRejectionEvent): void {
	const { name, message } = summarizeRejectionReason(event.reason);
	forward(`window.unhandledrejection:${name}`, message);
}

/** Registers the window-level listeners exactly once per process lifetime.
 * No-op outside a Tauri webview (plain Vite dev, lab.html) — there is no
 * backend to receive the records. */
export function initFrontendErrorLogBridge(): void {
	if (installed) return;
	if (!(window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__) return;
	installed = true;
	window.addEventListener('error', handleWindowError);
	window.addEventListener('unhandledrejection', handleUnhandledRejection);
}

/** Test-only: undoes `initFrontendErrorLogBridge` and clears rate-limit state
 * so test cases stay isolated from each other. Not part of the runtime path. */
export function disposeFrontendErrorLogBridgeForTests(): void {
	window.removeEventListener('error', handleWindowError);
	window.removeEventListener('unhandledrejection', handleUnhandledRejection);
	installed = false;
	windowStartMs = 0;
	sentInWindow = 0;
	droppedInWindow = 0;
}
