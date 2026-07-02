import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { getCurrentFileList, getSelectedFileIndices } from '../fileList';
import {
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	stageMetadataIntentPatch,
} from '../metadataSession';

/** Artifact fields ABB preserves on normal saves and exposes for explicit
 * inspect/clear only (#281). Distinct from primary audiobook form fields. */
export const METADATA_ARTIFACT_FIELDS = ['album_sort', 'comment', 'track', 'disk'] as const;
export type MetadataArtifactField = (typeof METADATA_ARTIFACT_FIELDS)[number];

const ARTIFACT_LABELS: Record<MetadataArtifactField, string> = {
	album_sort: 'Album sort (TSOA)',
	comment: 'Comment',
	track: 'Track number',
	disk: 'Disc number',
};

export type MetadataArtifactRow = {
	field: MetadataArtifactField;
	label: string;
	value: string | null;
	clearPending: boolean;
};

type MetadataArtifactsState = {
	filePath: string | null;
	rows: MetadataArtifactRow[];
	multiSelection: boolean;
};

export const metadataArtifactsState = $state<MetadataArtifactsState>({
	filePath: null,
	rows: [],
	multiSelection: false,
});

function formatPosition(value: unknown): string | null {
	if (!Array.isArray(value) || value.length === 0 || typeof value[0] !== 'number') {
		return null;
	}
	const number = value[0];
	const total = typeof value[1] === 'number' ? value[1] : null;
	if (number === 0) {
		return null;
	}
	return total != null ? `${number} of ${total}` : `${number}`;
}

function formatArtifactValue(
	metadata: Partial<AudiobookMetadata>,
	field: MetadataArtifactField,
): string | null {
	if (field === 'track' || field === 'disk') {
		return formatPosition(metadata[field]);
	}
	const value = metadata[field];
	if (typeof value !== 'string') {
		return null;
	}
	const trimmed = value.trim();
	return trimmed.length > 0 ? trimmed : null;
}

function clearIsPending(
	patch: MetadataIntentPatch | undefined,
	field: MetadataArtifactField,
): boolean {
	return patch?.[field]?.op === 'clear';
}

function buildRows(filePath: string): MetadataArtifactRow[] {
	const metadata = getMetadataForFile(filePath) ?? {};
	const patch = getMetadataIntentPatchForFile(filePath);
	return METADATA_ARTIFACT_FIELDS.map((field) => ({
		field,
		label: ARTIFACT_LABELS[field],
		value: formatArtifactValue(metadata, field),
		clearPending: clearIsPending(patch, field),
	}));
}

/** Re-reads the selected file's artifact values. Called from the FileList
 * selection presentation flow (like `updateTagPreview`) and after staging a
 * clear. */
export function refreshMetadataArtifacts(): void {
	const fileList = getCurrentFileList();
	const indices = Array.from(getSelectedFileIndices());
	if (!fileList || indices.length !== 1) {
		metadataArtifactsState.filePath = null;
		metadataArtifactsState.rows = [];
		metadataArtifactsState.multiSelection = indices.length > 1;
		return;
	}
	const index = indices[0] ?? -1;
	const file = fileList.files[index];
	if (!file?.isValid) {
		metadataArtifactsState.filePath = null;
		metadataArtifactsState.rows = [];
		metadataArtifactsState.multiSelection = false;
		return;
	}
	metadataArtifactsState.filePath = file.path;
	metadataArtifactsState.rows = buildRows(file.path);
	metadataArtifactsState.multiSelection = false;
}

/** Stages an explicit clear intent for one artifact field on the selected
 * file. Rides the normal pending-save mechanism: Cmd+S / Save sends it, and
 * saves without this intent keep preserving the field. */
export function stageMetadataArtifactClear(field: MetadataArtifactField): void {
	const filePath = metadataArtifactsState.filePath;
	if (!filePath) {
		return;
	}
	stageMetadataIntentPatch(filePath, { [field]: { op: 'clear' } });
	refreshMetadataArtifacts();
}
