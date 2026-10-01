/**
 * The engine's working session as the frontend receives it.
 *
 * Rust owns every shape here. Audio files and lookup results are converted
 * to the optional-field forms the rest of the frontend already uses; the
 * other parts are the generated shapes unchanged.
 */

import type {
	LookupSnapshot as GeneratedLookupSnapshot,
	MetadataSnapshot as GeneratedMetadataSnapshot,
	SelectionSnapshot as GeneratedSelectionSnapshot,
	SessionIntent as GeneratedSessionIntent,
	SessionOutcome as GeneratedSessionOutcome,
	TitlesSnapshot as GeneratedTitlesSnapshot,
} from '../lib/generated/tauri';
import type { AudioFile } from './audio';
import type { OnlineMetadataResult } from './metadata';

export type {
	CoverNotice,
	CoverSnapshot,
	FieldAction,
	FieldSnapshot,
	InputNotice,
	LookupApplyMode,
	LookupSource,
	LookupStatus,
	MetadataField,
	MetadataFormSnapshot,
	MetadataStatus,
	QueueStep,
	SeriesPartWarning,
	SubseriesPartWarning,
} from '../lib/generated/tauri';

export type SessionIntent = GeneratedSessionIntent;
export type SessionOutcome = GeneratedSessionOutcome;

export type SessionTitles = Omit<GeneratedTitlesSnapshot, 'files' | 'titleSourcesByIdentity'> & {
	files: AudioFile[];
	titleSourcesByIdentity: Record<string, AudioFile[]>;
};
export type SessionSelection = GeneratedSelectionSnapshot;
export type SessionMetadata = GeneratedMetadataSnapshot;
export type SessionLookup = Omit<GeneratedLookupSnapshot, 'results'> & {
	results: OnlineMetadataResult[];
};

/** The parts that changed. An absent part did not change. */
export type SessionUpdate = {
	revision: number;
	titles?: SessionTitles;
	selection?: SessionSelection;
	metadata?: SessionMetadata;
	lookup?: SessionLookup;
};

export type SessionReply = {
	outcome: SessionOutcome;
	update: SessionUpdate;
};
