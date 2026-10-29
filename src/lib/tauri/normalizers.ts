import type {
	FrontendAttachment as GeneratedFrontendAttachment,
	SessionReply as GeneratedSessionReply,
	OutputSnapshot as GeneratedOutputSnapshot,
	SessionUpdate as GeneratedSessionUpdate,
	SettingsReply as GeneratedSettingsReply,
	SettingsSnapshot as GeneratedSettingsSnapshot,
	ProcessCommandResult as GeneratedProcessCommandResult,
	WorkOperationsSnapshot as GeneratedWorkOperationsSnapshot,
	WorkOperationsUpdate as GeneratedWorkOperationsUpdate,
	OperationSnapshot as GeneratedOperationSnapshot,
} from '../generated/tauri';
import type { PlannedOutput, ProcessCommandResult } from '../../types/audio';
import type { SettingsReply, SettingsSnapshot } from '../../types/appSettings';
import type { SessionReply, SessionUpdate, SubmissionStatus } from '../../types/session';
import type { NullToOptionalDeep } from '../../types/ipc';
import type {
	OperationSnapshot,
	WorkOperationsSnapshot,
	WorkOperationsUpdate,
} from '../../types/workRuntime';
import { normalizeAppError } from './appError';

type PlainRecord = Record<string, unknown>;

const isPlainRecord = (value: unknown): value is PlainRecord =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

function isScalarArrayWithoutNullish(value: readonly unknown[]): boolean {
	for (const entry of value) {
		if (entry == null || typeof entry === 'object') {
			return false;
		}
	}
	return true;
}

function normalizeNullish<T>(value: T): NullToOptionalDeep<T> {
	if (value == null) {
		return undefined as NullToOptionalDeep<T>;
	}
	if (Array.isArray(value)) {
		if (isScalarArrayWithoutNullish(value)) {
			return value as NullToOptionalDeep<T>;
		}
		const normalized = new Array<unknown>(value.length);
		for (const [index, entry] of value.entries()) {
			normalized[index] = normalizeNullish(entry);
		}
		return normalized as NullToOptionalDeep<T>;
	}
	if (isPlainRecord(value)) {
		const normalized: PlainRecord = {};
		for (const [key, entryValue] of Object.entries(value)) {
			const converted = normalizeNullish(entryValue);
			if (converted !== undefined) {
				normalized[key] = converted;
			}
		}
		return normalized as NullToOptionalDeep<T>;
	}
	return value as NullToOptionalDeep<T>;
}

export function denormalizeNullish<T>(value: T): T {
	if (value === undefined) {
		return null as T;
	}
	if (Array.isArray(value)) {
		if (isScalarArrayWithoutNullish(value)) {
			return value as T;
		}
		const normalized = new Array<unknown>(value.length);
		for (const [index, entry] of value.entries()) {
			normalized[index] = denormalizeNullish(entry);
		}
		return normalized as T;
	}
	if (isPlainRecord(value)) {
		const normalized: PlainRecord = {};
		for (const [key, entryValue] of Object.entries(value)) {
			normalized[key] = denormalizeNullish(entryValue);
		}
		return normalized as T;
	}
	return value;
}

/**
 * Audio files and lookup results take the optional-field forms the frontend
 * uses elsewhere. Audio requests keep their explicit nulls: a null `settings`
 * is the request for MP3 pass-through.
 */
export function normalizeSessionUpdate(update: GeneratedSessionUpdate): SessionUpdate {
	const { titles, selection, metadata, lookup, audio, output, remote, remoteLibrary } = update;
	const { remoteAcquisition } = update;
	return {
		revision: update.revision,
		titles: titles
			? {
					...titles,
					files: normalizeNullish(titles.files),
					titleSourcesByIdentity: normalizeNullish(titles.titleSourcesByIdentity),
				}
			: undefined,
		selection: selection ?? undefined,
		remote: remote
			? {
					...remote,
					account: remote.account ? normalizeNullish(remote.account) : null,
					indexer: {
						...remote.indexer,
						releases: remote.indexer.releases.map((release) => ({
							...normalizeNullish(release),
							categories: release.categories,
						})),
					},
				}
			: undefined,
		remoteAcquisition: remoteAcquisition
			? {
					...remoteAcquisition,
					job: remoteAcquisition.job ? normalizeNullish(remoteAcquisition.job) : null,
				}
			: undefined,
		remoteLibrary: remoteLibrary
			? {
					...remoteLibrary,
					titles: normalizeNullish(remoteLibrary.titles),
					diagnostics: normalizeNullish(remoteLibrary.diagnostics),
				}
			: undefined,
		metadata: metadata ?? undefined,
		lookup: lookup ? { ...lookup, results: normalizeNullish(lookup.results) } : undefined,
		audio: audio ?? undefined,
		output: output
			? {
					...output,
					submission: normalizeSubmission(output.submission),
					collisionReview: output.collisionReview
						? {
								...output.collisionReview,
								outputs: normalizeNullish(output.collisionReview.outputs) as PlannedOutput[],
							}
						: null,
				}
			: undefined,
	};
}

function normalizeSubmission(
	submission: GeneratedOutputSnapshot['submission'],
): SubmissionStatus | null {
	switch (submission?.kind) {
		case 'previewFinished':
			return { kind: 'previewFinished', result: normalizeProcessResult(submission.result) };
		default:
			return submission ?? null;
	}
}

export function normalizeSessionReply(reply: GeneratedSessionReply): SessionReply {
	return { outcome: reply.outcome, update: normalizeSessionUpdate(reply.update) };
}

export function normalizeSettingsSnapshot(snapshot: GeneratedSettingsSnapshot): SettingsSnapshot {
	return normalizeNullish(snapshot) as SettingsSnapshot;
}

export function normalizeSettingsReply(reply: GeneratedSettingsReply): SettingsReply {
	return { outcome: reply.outcome, snapshot: normalizeSettingsSnapshot(reply.snapshot) };
}

export function normalizeFrontendAttachment(attachment: GeneratedFrontendAttachment): {
	client: number;
	session: SessionUpdate;
	settings: SettingsSnapshot;
} {
	return {
		client: attachment.client,
		session: normalizeSessionUpdate(attachment.session),
		settings: normalizeSettingsSnapshot(attachment.settings),
	};
}

function normalizeProcessResult(result: GeneratedProcessCommandResult): ProcessCommandResult {
	const normalized = normalizeNullish(result) as ProcessCommandResult;
	return {
		...normalized,
		results: (normalized.results ?? []).map((entry) => ({
			...entry,
			error: entry.error == null ? undefined : normalizeAppError(entry.error),
		})),
	};
}

export function normalizeOperationSnapshot(payload: GeneratedOperationSnapshot): OperationSnapshot {
	return normalizeNullish(payload) as OperationSnapshot;
}

export function normalizeWorkOperationsSnapshot(
	payload: GeneratedWorkOperationsSnapshot,
): WorkOperationsSnapshot {
	return normalizeNullish(payload) as WorkOperationsSnapshot;
}

export function normalizeWorkOperationsUpdate(
	payload: GeneratedWorkOperationsUpdate,
): WorkOperationsUpdate {
	return normalizeNullish(payload) as WorkOperationsUpdate;
}
