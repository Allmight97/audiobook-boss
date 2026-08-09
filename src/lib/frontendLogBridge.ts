import type { FrontendLogLevel } from '../types/frontendLog';
import { isAppErrorEnvelope } from './tauri/appError';
import { tauriClient } from './tauri/client';

const MESSAGE_MAX_CHARS = 500;
const SCOPE_MAX_CHARS = 200;
const MAX_EVENTS_PER_WINDOW = 20;
const RATE_LIMIT_WINDOW_MS = 60_000;

let installed = false;
let windowStartMs = 0;
let sentInWindow = 0;
let droppedInWindow = 0;

export function flattenLogText(value: string): string {
	return [...value.replace(/[\r\n\t]+/g, ' ')]
		.filter((character) => {
			const code = character.charCodeAt(0);
			return code > 0x1f && (code < 0x7f || code > 0x9f);
		})
		.join('');
}

function truncate(value: string, maxChars: number): string {
	return value.length <= maxChars ? value : `${value.slice(0, maxChars)}…`;
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
	try {
		void tauriClient
			.logFrontend({
				level,
				scope: sanitizeOutboundField(scope, SCOPE_MAX_CHARS),
				message: sanitizeOutboundField(message, MESSAGE_MAX_CHARS),
			})
			.catch(() => undefined);
	} catch {
		// Plain Vite and the design lab have no Tauri backend.
	}
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

export function disposeFrontendErrorLogBridgeForTests(): void {
	window.removeEventListener('error', handleWindowError);
	window.removeEventListener('unhandledrejection', handleUnhandledRejection);
	installed = false;
	windowStartMs = 0;
	sentInWindow = 0;
	droppedInWindow = 0;
}
