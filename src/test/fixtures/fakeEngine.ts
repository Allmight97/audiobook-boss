import { recordedFresh } from '../../lib/tauri/recordedFresh';
import type { EngineCapability } from '../../lib/tauri/capabilities/engine';
import type {
	AppSettings,
	SettingsIntent,
	SettingsOutcome,
	SettingsSnapshot,
} from '../../types/appSettings';
import type { AudioFile, TitleAudioRequest } from '../../types/audio';
import type {
	CoverNotice,
	MetadataField,
	MetadataStatus,
	RemoteAcquisitionSnapshot,
	RemoteLibrarySnapshot,
	RemoteUiSnapshot,
	SessionAudio,
	SessionIntent,
	SessionLookup,
	SessionMetadata,
	SessionOutcome,
	SessionOutput,
	SessionSelection,
	SessionTitles,
	SessionUpdate,
	SizeEstimate,
	TitleAudio,
	TitlePlan,
} from '../../types/session';
import type { OperationId, OperationSnapshot, WorkOperationsUpdate } from '../../types/workRuntime';

const FORM_FIELDS: readonly MetadataField[] = [
	'title',
	'date',
	'author',
	'narrator',
	'series',
	'seriesPart',
	'subseries',
	'subseriesPart',
	'genre',
	'description',
];

type SessionParts = {
	titles: SessionTitles;
	selection: SessionSelection;
	metadata: SessionMetadata;
	lookup: SessionLookup;
	audio: SessionAudio;
	output: SessionOutput;
	remote: RemoteUiSnapshot;
	remoteLibrary: RemoteLibrarySnapshot;
	remoteAcquisition: RemoteAcquisitionSnapshot;
};

function freshSession(): SessionParts {
	const { revision: _revision, ...parts } = structuredClone(recordedFresh.session);
	return parts as SessionParts;
}

export function defaultAppSettings(): AppSettings {
	return structuredClone(recordedFresh.settings.settings);
}

export function sessionUpdate(
	parts: Pick<SessionUpdate, 'revision'> & Partial<SessionUpdate>,
): SessionUpdate {
	return {
		titles: null,
		selection: null,
		metadata: null,
		lookup: null,
		audio: null,
		output: null,
		remote: null,
		remoteLibrary: null,
		remoteAcquisition: null,
		...parts,
	};
}

export function audioFile(path: string, overrides: Partial<AudioFile> = {}): AudioFile {
	const isValid = overrides.isValid ?? true;
	return {
		inputId: path,
		path,
		label: path.split('/').findLast(Boolean) ?? path,
		size: 1024,
		duration: 60,
		format: 'm4b',
		bitrate: null,
		sampleRate: null,
		channels: null,
		codecLabel: null,
		selectedDecoder: null,
		artist: null,
		chapterPlan: null,
		cueSource: null,
		error: null,
		isValid,
		sourceRemoved: false,
		chapters: [],
		cover: isValid ? `audio/0/${encodeURIComponent(path)}` : null,
		...overrides,
	};
}

export type FakeEngine = EngineCapability & {
	readonly sessionIntents: SessionIntent[];
	readonly settingsIntents: SettingsIntent[];
	readonly claimedPreviews: string[];
	readonly openedPreviews: string[];
	readonly cancelledWork: { operationId: OperationId; childJobId?: string }[];
	/** Answers a cancel; a rejection fails it. */
	respondCancel?: () => Promise<void>;
	/** Tells every listener the Quit shortcut was pressed once. */
	pressQuitOnce(windowMs: number): void;
	/** Adds or replaces an operation and tells every listener. */
	publishOperation(operation: OperationSnapshot): void;
	analyze: (paths: readonly string[]) => AudioFile[];
	/**
	 * Answers a session intent. A reply of `undefined`, at once or later, is
	 * `applied`.
	 */
	respond?: (
		intent: SessionIntent,
	) => SessionOutcome | Promise<SessionOutcome | undefined> | undefined;
	/** Answers a preview-output claim with the path to open; `undefined` is no path. */
	respondPreviewOutput?: (runId: string) => string | Promise<string | undefined> | undefined;
	/** Answers a settings intent; `undefined` is `applied`. */
	respondSettings?: (
		intent: SettingsIntent,
	) => SettingsOutcome | Promise<SettingsOutcome | undefined> | undefined;
	/** Replaces parts of the settings snapshot and tells every listener. */
	seedSettings(patch: Partial<SettingsSnapshot>): void;
	/** Changes saved settings, with the reason they are not on disk, if any. */
	seedAppSettings(patch: Partial<AppSettings>, saveError?: SettingsSnapshot['saveError']): void;
	seedGroup(sources: AudioFile[], options?: { readonly choiceRequired?: boolean }): void;
	/** Sets the metadata form to these values, every field unchanged and shared. */
	seedForm(
		values: Partial<Record<MetadataField, string>>,
		options?: {
			readonly mode?: 'single' | 'multi';
			readonly selectionCount?: number;
		},
	): void;
	seedCover(notice?: CoverNotice): void;
	seedTitleAudio(
		titleId: string,
		request: TitleAudioRequest,
		resolved?: {
			readonly plan?: TitlePlan;
			readonly estimate?: SizeEstimate | null;
		},
	): void;
	loadTitles(files: AudioFile[], selected?: number[]): void;
	status(status: MetadataStatus | null): void;
	settings(): SettingsSnapshot;
	change(mutate: (state: SessionParts) => void): void;
};

export function createFakeEngine(initialSettings: AppSettings = defaultAppSettings()): FakeEngine {
	let revision = 0;
	const listeners = new Set<(update: SessionUpdate) => void>();
	const settingsListeners = new Set<(snapshot: SettingsSnapshot) => void>();
	const quitPressListeners = new Set<(windowMs: number) => void>();
	const workListeners = new Set<(update: WorkOperationsUpdate) => void>();
	const operations = new Map<OperationId, OperationSnapshot>();
	let workRevision = 0;
	const state = freshSession();
	let settingsSnapshot: SettingsSnapshot = {
		...structuredClone(recordedFresh.settings),
		settings: structuredClone(initialSettings),
	};
	let coverRevision = 0;
	let nextSessionSequence = 0;
	let nextSettingsSequence = 0;

	function titleFromDefaults(): TitleAudio {
		return {
			...structuredClone(state.audio.defaults),
			plan: { kind: 'pending' },
			estimate: null,
			autoSampleRate: { outcome: { kind: 'unresolved' }, problem: null },
			autoChannels: { outcome: { kind: 'unresolved' }, problem: null },
		};
	}

	function settle(): SessionUpdate {
		revision += 1;
		for (const part of Object.values(state)) part.revision = revision;
		return structuredClone({ revision, ...state });
	}

	function publish(): void {
		const update = settle();
		for (const listener of listeners) listener(update);
	}

	function select(indices: number[]): void {
		state.selection = {
			...state.selection,
			selectedIndices: [...indices].toSorted((a, b) => a - b),
			selectedAnchor: indices.length ? indices[indices.length - 1] : null,
		};
	}

	function appendFiles(files: AudioFile[]): void {
		state.titles.files = [...state.titles.files, ...files];
		for (const file of files) {
			if (file.inputId) state.audio.titles[file.inputId] = titleFromDefaults();
		}
	}

	function takeSessionTurn(sequence: number): void {
		if (sequence !== nextSessionSequence)
			throw new Error(`session intent ${sequence} arrived out of order`);
		nextSessionSequence += 1;
	}

	const engine: FakeEngine = {
		sessionIntents: [],
		settingsIntents: [],
		claimedPreviews: [],
		openedPreviews: [],
		cancelledWork: [],
		analyze: (paths) => paths.map((path) => audioFile(path)),
		async attach() {
			nextSessionSequence = 0;
			nextSettingsSequence = 0;
			return {
				client: 1,
				session: settle(),
				settings: structuredClone(settingsSnapshot),
				workOperations: structuredClone({
					revision: workRevision,
					order: [...operations.keys()],
					operations: [...operations.values()],
				}),
			};
		},
		async sessionDispatch(_client, sequence, intent) {
			takeSessionTurn(sequence);
			engine.sessionIntents.push(structuredClone(intent));
			const answered = await engine.respond?.(intent);
			if (!answered && intent.kind === 'import') appendFiles(engine.analyze(intent.paths));
			return { outcome: answered ?? { kind: 'applied' }, update: settle() };
		},
		async openPreviewOutput(_client, sequence, runId) {
			takeSessionTurn(sequence);
			engine.claimedPreviews.push(runId);
			const path = (await engine.respondPreviewOutput?.(runId)) ?? null;
			if (path) engine.openedPreviews.push(path);
			return { outcome: { kind: 'previewOutput', path }, update: settle() };
		},
		async settingsDispatch(_client, sequence, intent) {
			if (sequence !== nextSettingsSequence)
				throw new Error(`settings intent ${sequence} arrived out of order`);
			nextSettingsSequence += 1;
			engine.settingsIntents.push(structuredClone(intent));
			const outcome = (await engine.respondSettings?.(intent)) ?? {
				kind: 'applied',
			};
			return { outcome, snapshot: structuredClone(settingsSnapshot) };
		},
		async listenSessionUpdates(handler) {
			listeners.add(handler);
			return () => listeners.delete(handler);
		},
		async listenSettingsUpdates(handler) {
			settingsListeners.add(handler);
			return () => settingsListeners.delete(handler);
		},
		async listenQuitPressAgain(handler) {
			quitPressListeners.add(handler);
			return () => quitPressListeners.delete(handler);
		},
		async listenWorkOperations(handler) {
			workListeners.add(handler);
			return () => workListeners.delete(handler);
		},
		async cancelWorkOperation(operationId, childJobId) {
			engine.cancelledWork.push({ operationId, childJobId });
			await engine.respondCancel?.();
		},
		publishOperation(operation) {
			operations.set(operation.operationId, structuredClone(operation));
			workRevision += 1;
			const update = {
				revision: workRevision,
				order: [...operations.keys()],
				changed: operation,
			};
			for (const listener of workListeners) listener(structuredClone(update));
		},
		pressQuitOnce(windowMs) {
			for (const listener of quitPressListeners) listener(windowMs);
		},
		seedSettings(patch) {
			settingsSnapshot = {
				...settingsSnapshot,
				...structuredClone(patch),
				revision: settingsSnapshot.revision + 1,
			};
			for (const listener of settingsListeners) listener(structuredClone(settingsSnapshot));
		},
		seedAppSettings(patch, saveError) {
			engine.seedSettings({
				settings: { ...settingsSnapshot.settings, ...patch },
				saveError,
			});
		},
		seedGroup(sources, options = {}) {
			const [anchor] = sources;
			const id = anchor?.inputId;
			if (!id) throw new Error('a group needs an anchor with an input id');
			const members = new Set(sources.map((file) => file.path));
			const titles = state.titles;
			titles.files = titles.files.filter(
				(file) => file.path === anchor.path || !members.has(file.path),
			);
			titles.titleSourcesByIdentity = {
				...titles.titleSourcesByIdentity,
				[id]: sources,
			};
			if (options.choiceRequired)
				state.audio.titles = {
					...state.audio.titles,
					[id]: {
						...(state.audio.titles[id] ?? titleFromDefaults()),
						plan: { kind: 'choiceRequired' },
					},
				};
			select([titles.files.findIndex((file) => file.path === anchor.path)]);
			publish();
		},
		seedForm(values, options = {}) {
			state.metadata.form = {
				...state.metadata.form,
				mode: options.mode ?? 'single',
				selectionCount: options.selectionCount ?? 0,
				fields: FORM_FIELDS.map((field) => ({
					field,
					value: values[field] ?? '',
					action: 'keep',
					dirty: false,
					mixed: false,
				})),
			};
			publish();
		},
		seedCover(notice) {
			const cover = state.metadata.cover;
			coverRevision += 1;
			state.metadata.cover = {
				...cover,
				image: `session/${coverRevision}/`,
				custom: true,
				notice: notice ?? cover.notice,
				noticeSerial: notice ? cover.noticeSerial + 1 : cover.noticeSerial,
			};
			publish();
		},
		loadTitles(files, selected = []) {
			appendFiles(files);
			if (selected.length) select(selected);
			publish();
		},
		status(status) {
			state.metadata.status = status;
			publish();
		},
		seedTitleAudio(titleId, request, resolved = {}) {
			const view = structuredClone(state.audio.titles[titleId] ?? titleFromDefaults());
			view.request = structuredClone(request);
			view.choice = {
				...view.choice,
				format: request.format,
				intent: request.intent,
			};
			if (resolved.plan) view.plan = resolved.plan;
			if (resolved.estimate !== undefined) view.estimate = resolved.estimate;
			state.audio.titles = { ...state.audio.titles, [titleId]: view };
			publish();
		},
		settings: () => structuredClone(settingsSnapshot),
		change(mutate) {
			mutate(state);
			publish();
		},
	};

	return engine;
}

let current = createFakeEngine();

export function fakeEngine(): FakeEngine {
	return current;
}

export function resetFakeEngine(): void {
	current = createFakeEngine();
}
