/**
 * The engine's working session as the frontend receives it.
 *
 * Rust owns every shape here. Audio files and lookup results are converted
 * to the optional-field forms the rest of the frontend already uses; the
 * other parts are the generated shapes unchanged.
 */

import type {
	AudioSnapshot as GeneratedAudioSnapshot,
	LookupSnapshot as GeneratedLookupSnapshot,
	OutputSnapshot as GeneratedOutputSnapshot,
	SubmissionStatus as GeneratedSubmissionStatus,
	MetadataSnapshot as GeneratedMetadataSnapshot,
	SelectionSnapshot as GeneratedSelectionSnapshot,
	SessionIntent as GeneratedSessionIntent,
	SessionOutcome as GeneratedSessionOutcome,
	TitlesSnapshot as GeneratedTitlesSnapshot,
} from '../lib/generated/tauri';
import type { AudioFile, PlannedOutput, ProcessCommandResult } from './audio';
import type { OnlineMetadataResult } from './metadata';

export type {
	AudioChoice,
	AudioChoiceFacts,
	AudioChoiceView,
	AudioEdit,
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
	FaacRateControl,
	OutputPreview,
	SizeEstimate,
	SubmitRefusal,
	TagPreview,
	TitleAudio,
	TitlePlan,
} from '../lib/generated/tauri';

export type SessionIntent = GeneratedSessionIntent;
export type SessionOutcome = GeneratedSessionOutcome;

export type SessionTitles = Omit<GeneratedTitlesSnapshot, 'files' | 'titleSourcesByIdentity'> & {
	files: AudioFile[];
	titleSourcesByIdentity: Record<string, AudioFile[]>;
};
export type SessionSelection = GeneratedSelectionSnapshot;
export type SessionMetadata = GeneratedMetadataSnapshot;
/** The audio part keeps the generated shape: an MP3 request's null settings are meaningful. */
export type SessionAudio = GeneratedAudioSnapshot;
/** Planned outputs and a finished preview's result take the frontend's optional-field forms. */
export type SubmissionStatus =
	| Exclude<GeneratedSubmissionStatus, { kind: 'previewFinished' | 'reviewRequired' }>
	| { kind: 'reviewRequired'; outputs: PlannedOutput[]; preview: boolean }
	| { kind: 'previewFinished'; result: ProcessCommandResult };
export type SessionOutput = Omit<GeneratedOutputSnapshot, 'submission'> & {
	submission: SubmissionStatus | null;
};
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
	audio?: SessionAudio;
	output?: SessionOutput;
};

export type SessionReply = {
	outcome: SessionOutcome;
	update: SessionUpdate;
};
