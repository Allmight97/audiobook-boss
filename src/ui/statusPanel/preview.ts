import { tauriClient } from '../../lib/tauri/client';
import type { ProcessCommandJobResult, ProcessCommandResult } from '../../types/audio';

function isSuccessfulResultEntry(entry: ProcessCommandJobResult): boolean {
	return entry.status === 'success';
}

function extractSuccessfulPreviewPaths(result: ProcessCommandResult): string[] {
	return result.results
		.filter(
			(entry) => typeof entry.previewFilePath === 'string' && entry.previewFilePath.length > 0,
		)
		.filter(isSuccessfulResultEntry)
		.map((entry) => entry.previewFilePath as string);
}

export async function openGeneratedPreviewIfSingle(result: ProcessCommandResult): Promise<void> {
	const previewPaths = extractSuccessfulPreviewPaths(result);
	if (previewPaths.length !== 1) {
		return;
	}

	const [previewPath] = previewPaths;
	const successfulPreview = result.results.find(
		(entry) => entry.previewFilePath === previewPath && isSuccessfulResultEntry(entry),
	);
	const seconds =
		typeof successfulPreview?.previewActualSeconds === 'number'
			? successfulPreview.previewActualSeconds.toFixed(3)
			: '≈30';
	console.log(`Preview file created at: ${previewPath} (${seconds}s)`);

	try {
		await tauriClient.openExternal(previewPath);
	} catch (error) {
		console.warn('Failed to open preview file automatically:', error);
	}
}
