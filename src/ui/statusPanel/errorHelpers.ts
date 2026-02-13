export function normalizeProcessingErrorMessage(
	error: unknown,
	fallback = 'Unknown error',
): string {
	if (error instanceof Error) {
		return error.message;
	}
	if (typeof error === 'string') {
		return error;
	}
	if (typeof error === 'object' && error !== null && 'message' in error) {
		const message = (error as { message?: unknown }).message;
		if (typeof message === 'string') {
			return message;
		}
	}
	if (error == null) {
		return fallback;
	}
	return String(error);
}
