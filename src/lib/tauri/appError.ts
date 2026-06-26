type PlainRecord = Record<string, unknown>;

export const APP_ERROR_CATEGORIES = [
	'validation',
	'cancellation',
	'toolchain',
	'resource',
	'io',
	'internal',
	'auth',
	'network',
	'import',
	'processing',
	'unknown',
] as const;

export type AppErrorCategory = (typeof APP_ERROR_CATEGORIES)[number] | (string & {});

export interface AppErrorEnvelope {
	code: string;
	category: AppErrorCategory;
	message: string;
	detail?: string | null;
}

function isPlainRecord(value: unknown): value is PlainRecord {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function toStringValue(value: unknown, fallback: string): string {
	if (typeof value === 'string') {
		return value;
	}
	if (value == null) {
		return fallback;
	}
	return String(value);
}

export function isAppErrorEnvelope(value: unknown): value is AppErrorEnvelope {
	if (!isPlainRecord(value)) {
		return false;
	}

	return (
		typeof value.code === 'string' &&
		typeof value.category === 'string' &&
		typeof value.message === 'string' &&
		(value.detail === undefined || value.detail === null || typeof value.detail === 'string')
	);
}

export function normalizeAppError(
	error: unknown,
	fallbackMessage = 'Unknown error',
): AppErrorEnvelope {
	if (isAppErrorEnvelope(error)) {
		return error;
	}

	if (error instanceof Error) {
		return {
			code: 'unknown_error',
			category: 'unknown',
			message: error.message || fallbackMessage,
			detail: error.stack || undefined,
		};
	}

	if (typeof error === 'string') {
		return {
			code: 'unknown_error',
			category: 'unknown',
			message: error || fallbackMessage,
		};
	}

	if (isPlainRecord(error)) {
		const message = toStringValue(error.message, fallbackMessage);
		return {
			code: toStringValue(error.code, 'unknown_error'),
			category: toStringValue(error.category, 'unknown') as AppErrorCategory,
			message,
			detail: typeof error.detail === 'string' ? error.detail : undefined,
		};
	}

	return {
		code: 'unknown_error',
		category: 'unknown',
		message: fallbackMessage,
	};
}

/**
 * Canonical Result shape produced by specta-generated command bindings:
 *
 *     { status: "ok"; data: T } | { status: "error"; error: E }
 *
 * See `src/lib/generated/tauri.ts`'s `export type Result<T, E>` for the
 * authoritative declaration. Anything else returned from a generated command
 * is a bare value (e.g. `get_max_concurrent_jobs: number`) and flows through
 * unchanged.
 */
type SpectaResult = { status: 'ok'; data?: unknown } | { status: 'error'; error?: unknown };

function isSpectaResult(value: unknown): value is SpectaResult {
	if (!isPlainRecord(value)) {
		return false;
	}
	return value.status === 'ok' || value.status === 'error';
}

export function unwrapGeneratedResult<T>(value: unknown): T {
	if (!isSpectaResult(value)) {
		return value as T;
	}

	if (value.status === 'error') {
		throw normalizeAppError(value.error);
	}

	return value.data as T;
}

export function isAppErrorCategory(error: unknown, category: AppErrorCategory): boolean {
	return normalizeAppError(error).category === category;
}

/**
 * Derive a user-facing message from any thrown cause. Centralizes the
 * `normalizeAppError(...).message` derivation the UI islands re-implement.
 * With `suppressUnknown`, an unclassified (`unknown_error`) cause collapses to
 * `fallback` instead of surfacing a raw/internal message.
 */
export function toUserMessage(
	cause: unknown,
	options: { fallback?: string; suppressUnknown?: boolean } = {},
): string {
	const fallback = options.fallback ?? 'Unknown error';
	const normalized = normalizeAppError(cause, fallback);
	if (options.suppressUnknown && normalized.code === 'unknown_error') {
		return fallback;
	}
	return normalized.message || fallback;
}

/**
 * True when a cause represents a cancellation. Prefers the typed
 * `category: 'cancellation'`, and keeps a message fallback because cancellations
 * can still arrive un-enveloped (a raw error normalizes to category `unknown`).
 * Proving cancellations are always typed is owned by the lifecycle-truth work
 * (#376); until then this is the single owner of that dual check.
 */
export function isCancellation(cause: unknown): boolean {
	const normalized = normalizeAppError(cause);
	return (
		normalized.category === 'cancellation' || normalized.message.toLowerCase().includes('cancelled')
	);
}

/**
 * Log a normalized error with a consistent `code=/category=` shape. Defaults to
 * `console.error`; pass `level: 'warn'` for non-fatal diagnostics.
 */
export function logAppError(
	scope: string,
	cause: unknown,
	level: 'error' | 'warn' = 'error',
): void {
	const error = normalizeAppError(cause);
	const line = `${scope} code=${error.code} category=${error.category}`;
	if (level === 'warn') {
		console.warn(line);
	} else {
		console.error(line);
	}
}
