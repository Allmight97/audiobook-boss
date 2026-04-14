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

function isResultRecord(value: unknown): value is PlainRecord {
	return (
		isPlainRecord(value) && ('Ok' in value || 'Err' in value || 'ok' in value || 'status' in value)
	);
}

export function unwrapGeneratedResult<T>(value: unknown): T {
	if (!isResultRecord(value)) {
		return value as T;
	}

	if ('Err' in value && value.Err !== undefined) {
		throw normalizeAppError(value.Err);
	}

	if ('Ok' in value) {
		return value.Ok as T;
	}

	if (value.status === 'error') {
		const errorValue = 'error' in value ? value.error : value;
		throw normalizeAppError(errorValue);
	}

	if (value.status === 'ok') {
		return ('data' in value ? value.data : undefined) as T;
	}

	const okValue = value.ok;
	if (typeof okValue === 'boolean') {
		if (!okValue) {
			const errorValue =
				'value' in value
					? value.value
					: 'error' in value
						? value.error
						: 'Err' in value
							? value.Err
							: value;
			throw normalizeAppError(errorValue);
		}

		if ('value' in value) {
			return value.value as T;
		}
		if ('data' in value) {
			return value.data as T;
		}
		if ('result' in value) {
			return value.result as T;
		}
		return undefined as T;
	}

	return value as T;
}

export function isAppErrorCategory(error: unknown, category: AppErrorCategory): boolean {
	return normalizeAppError(error).category === category;
}
