import { formatDuration } from '../../types/audio';
import { coverArtBytesToDataUrl } from '../coverArt';
import {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFiles,
	readActiveFileChapters,
	readInspectorFacts,
} from '../fileList';
import { readMetadataFormViewSnapshot } from '../metadataForm';
import { getMetadataForFile } from '../metadataSession';

type ActiveFileSummary = {
	hasSelection: boolean;
	heading: string;
	railSubtitle: string | null;
	coverDataUrl: string | null;
};

export function readActiveFileSummary(): ActiveFileSummary {
	const activeFile = getCurrentFileList()?.files[getSelectedFileIndex()] ?? null;
	const activeMetadata = activeFile ? getMetadataForFile(activeFile.path) : undefined;
	const facts = readInspectorFacts();
	const form = readMetadataFormViewSnapshot();
	const isMultiSelection = form.mode === 'multi' && form.selectionCount > 1;
	const title =
		activeMetadata?.title || facts.find((fact) => fact.label === 'File')?.value || 'Metadata';

	return {
		hasSelection: getSelectedFiles().length > 0 && activeFile !== null,
		heading: isMultiSelection ? `${form.selectionCount} files selected` : title,
		railSubtitle: isMultiSelection
			? null
			: `${activeMetadata?.artist || '—'} · ${formatDuration(activeFile?.duration)} · ${readActiveFileChapters().length} chapters`,
		coverDataUrl: activeMetadata?.cover_art?.length
			? coverArtBytesToDataUrl(activeMetadata.cover_art)
			: null,
	};
}
