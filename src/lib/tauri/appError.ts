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

export function formatAppErrorMessage(error: unknown, fallbackMessage = 'Unknown error'): string {
	return normalizeAppError(error, fallbackMessage).message;
}

/**
 * Canonical Result shape produced by specta-generated command bindings:
 *
 *     { status: "ok"; data: T } | { status: "error"; error: E }
 *
 * See `src/lib/generated/tauri.ts`'s `export type Result<T, E>` for the
 * authoritative declaration. Anything else returned from a generated command
 * is a bare value (e.g. `get_max_concurrent_jobs: number`,
 * `list_available_encoders: EncoderAvailability`) and flows through unchanged.
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
