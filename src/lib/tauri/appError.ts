import type { AppErrorCategory as WireErrorCategory } from '../generated/tauri';

type PlainRecord = Record<string, unknown>;

const APP_ERROR_CATEGORIES = [
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
	if (typeof value === 'number' || typeof value === 'boolean' || typeof value === 'bigint') {
		return String(value);
	}
	return fallback;
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
			category: toStringValue(error.category, 'unknown'),
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

export function unwrapGeneratedResult<T>(
	value: { status: 'ok'; data: T } | { status: 'error'; error: unknown },
): T {
	if (value.status === 'error') {
		throw normalizeAppError(value.error);
	}
	return value.data;
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

export function isCancellation(cause: unknown): boolean {
	return normalizeAppError(cause).category === ('cancellation' satisfies WireErrorCategory);
}

/**
 * Log a normalized error with a consistent `code=/category=` shape. The
 * message and detail stay out of the console line because they can carry paths
 * or provider text.
 */
export function logAppError(scope: string, cause: unknown): void {
	const error = normalizeAppError(cause);
	console.error(`${scope} code=${error.code} category=${error.category}`);
}
