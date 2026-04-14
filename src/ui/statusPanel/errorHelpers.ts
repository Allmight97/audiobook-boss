import {
	isAppErrorCategory,
	normalizeAppError,
	type AppErrorEnvelope,
} from '../../lib/tauri/appError';

export type { AppErrorEnvelope };

export function normalizeProcessingErrorMessage(
	error: unknown,
	fallback = 'Unknown error',
): string {
	return normalizeAppError(error, fallback).message;
}

export function isProcessingCancellationError(error: unknown): boolean {
	return isAppErrorCategory(error, 'cancellation');
}
