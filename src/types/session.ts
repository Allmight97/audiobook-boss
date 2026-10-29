import type {
	AudioSnapshot as GeneratedAudioSnapshot,
	CollisionReview as GeneratedCollisionReview,
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
import type {
	AcquisitionJob,
	RemoteRelease,
	RemoteTitle,
	RemoteSourceAccountState,
} from './remoteSource';
import type {
	RemoteAcquisitionSnapshot as GeneratedRemoteAcquisitionSnapshot,
	RemoteLibrarySnapshot as GeneratedRemoteLibrarySnapshot,
	RemoteUiSnapshot as GeneratedRemoteUiSnapshot,
} from '../lib/generated/tauri';
import type { OnlineMetadataResult } from './metadata';

export type {
	AudioChoice,
	AudioChoiceFacts,
	AudioChoiceView,
	AudioEdit,
	AudioField,
	AudioRefusal,
	SelectionAudio,
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
	OutputEdits,
	OutputPreview,
	RestartOffer,
	SizeEstimate,
	SubmitRefusal,
	TagPreview,
	RemoteUiIntent,
	IndexerDraftSnapshot,
	RemoteDraftStatus,
	TitleAudio,
	TitlePlan,
} from '../lib/generated/tauri';

export type SessionIntent = GeneratedSessionIntent;
export type SessionOutcome = GeneratedSessionOutcome;
export type RemoteUiSnapshot = Omit<GeneratedRemoteUiSnapshot, 'indexer' | 'account'> & {
	account: RemoteSourceAccountState | null;
	indexer: Omit<GeneratedRemoteUiSnapshot['indexer'], 'releases'> & { releases: RemoteRelease[] };
};

export type RemoteAcquisitionSnapshot = Omit<GeneratedRemoteAcquisitionSnapshot, 'job'> & {
	job: AcquisitionJob | null;
};

export type RemoteLibrarySnapshot = Omit<
	GeneratedRemoteLibrarySnapshot,
	'titles' | 'diagnostics'
> & { titles: RemoteTitle[]; diagnostics: AcquisitionJob['diagnostics'] };

export type SessionTitles = Omit<GeneratedTitlesSnapshot, 'files' | 'titleSourcesByIdentity'> & {
	files: AudioFile[];
	titleSourcesByIdentity: Record<string, AudioFile[]>;
};
export type SessionSelection = GeneratedSelectionSnapshot;
export type SessionMetadata = GeneratedMetadataSnapshot;
/** The audio part keeps the generated shape: an MP3 request's null settings are meaningful. */
export type SessionAudio = GeneratedAudioSnapshot;
export type SubmissionStatus =
	| Exclude<GeneratedSubmissionStatus, { kind: 'previewFinished' }>
	| { kind: 'previewFinished'; result: ProcessCommandResult };
export type CollisionReview = Omit<GeneratedCollisionReview, 'outputs'> & {
	outputs: PlannedOutput[];
};
export type SessionOutput = Omit<GeneratedOutputSnapshot, 'submission' | 'collisionReview'> & {
	submission: SubmissionStatus | null;
	collisionReview: CollisionReview | null;
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
	remote?: RemoteUiSnapshot;
	remoteLibrary?: RemoteLibrarySnapshot;
	remoteAcquisition?: RemoteAcquisitionSnapshot;
};

export type SessionReply = {
	outcome: SessionOutcome;
	update: SessionUpdate;
};
