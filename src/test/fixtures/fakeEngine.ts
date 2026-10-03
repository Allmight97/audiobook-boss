/**
 * A stand-in for the engine's session and settings, for frontend tests.
 *
 * The engine's rules are proven in Rust. This fake only keeps enough state
 * for the frontend to render something and to show which intents it sent; a
 * test that needs a specific engine answer sets it up through `respond` or
 * the seed methods.
 */

import type { EngineCapability } from '../../lib/tauri/capabilities/engine';
import type {
	AppSettings,
	SettingsIntent,
	SettingsOutcome,
	SettingsSnapshot,
} from '../../types/appSettings';
import type { AudioFile, ProcessCommandResult, TitleAudioRequest } from '../../types/audio';
import { runtimeSettingsCapabilitiesFixture } from './runtimeSettingsCapabilities';
import type { AudiobookMetadata } from '../../types/metadata';
import type {
	CoverNotice,
	FieldSnapshot,
	MetadataField,
	MetadataStatus,
	SessionIntent,
	SessionAudio,
	SessionLookup,
	SessionOutput,
	SizeEstimate,
	TitleAudio,
	TitlePlan,
	SessionMetadata,
	SessionOutcome,
	SessionSelection,
	SessionTitles,
	SessionUpdate,
	SubmissionStatus,
} from '../../types/session';

const FIELD_TAGS: ReadonlyArray<readonly [MetadataField, keyof AudiobookMetadata]> = [
	['title', 'title'],
	['date', 'date'],
	['author', 'artist'],
	['narrator', 'composer'],
	['series', 'series'],
	['seriesPart', 'series_part'],
	['subseries', 'subseries'],
	['subseriesPart', 'subseries_part'],
	['genre', 'genre'],
	['description', 'description'],
];

export function defaultAppSettings(): AppSettings {
	return {
		keepAwakeWhileWorking: true,
		maxConcurrentJobs: { mode: 'auto' },
		encoderDefaults: {
			format: 'm4b',
			intent: 'auto',
			sampleRate: 'auto',
			settings: {
				encoderType: 'auto',
				bitrateKbps: 64,
				bitrateMode: { mode: 'cbr' },
				channels: 'auto',
				nativeAacSpeed: 0,
				faacProfile: 'auto',
			},
		},
		outputDefaults: {
			outputNaming: { preset: 'absDefault', includeYear: false },
		},
		startupBehavior: 'rememberLastState',
		defaultAcquisitionLane: 'audible',
	};
}

/** An analyzed audio file as the engine would report it. */
export function audioFile(path: string, overrides: Partial<AudioFile> = {}): AudioFile {
	return {
		inputId: path,
		path,
		size: 1024,
		duration: 60,
		format: 'm4b',
		isValid: true,
		chapters: [],
		...overrides,
	};
}

type Rejection = { readonly message: string };

export type FakeEngine = EngineCapability & {
	/** Every session intent received, in order. */
	readonly sessionIntents: SessionIntent[];
	readonly settingsIntents: SettingsIntent[];
	/** What reading each file's tags yields. */
	readonly tags: Map<string, Partial<AudiobookMetadata>>;
	/** What importing a set of paths yields. Defaults to one valid file per path. */
	analyze: (paths: readonly string[]) => AudioFile[];
	/** Paths the operating system asked the app to open. */
	openedPaths: string[];
	/** Answers an intent in place of the fake's default behavior. */
	respond?: (intent: SessionIntent) => SessionOutcome | Promise<SessionOutcome> | undefined;
	/**
	 * How the engine answers a submit, preview, or collision choice. Defaults
	 * to an accepted export, a finished preview, and a cancelled review.
	 */
	answerSubmission: (intent: SessionIntent) => SubmissionStatus;
	/** Fails the next settings writes, as a full disk would. */
	settingsWriteError?: Rejection;
	/** Refuses the next concurrency change, as running jobs would. */
	concurrencyError?: Rejection;
	/** Records settings the way the engine does after a session edit, and announces them. */
	recordSettings(patch: Partial<AppSettings>): void;
	/**
	 * Groups listed titles under the first, as the engine would after
	 * grouping them; `choiceRequired` marks their audio as disagreeing.
	 */
	seedGroup(sources: AudioFile[], options?: { readonly choiceRequired?: boolean }): void;
	/** Shows `value` as an edit on the bound form, as the engine would after applying one. */
	seedField(field: MetadataField, value: string): void;
	/** Shows a chosen cover image as the engine would after loading it. */
	seedCover(bytes: number[], notice?: CoverNotice): void;
	/** Sets a title's audio as the engine would after an edit and its plan. */
	seedTitleAudio(
		titleId: string,
		request: TitleAudioRequest,
		resolved?: { readonly plan?: TitlePlan; readonly estimate?: SizeEstimate | null },
	): void;
	/** Loads titles as an import would. */
	loadTitles(files: AudioFile[], selected?: number[]): void;
	status(status: MetadataStatus | null): void;
	titles(): SessionTitles;
	selection(): SessionSelection;
	metadata(): SessionMetadata;
	lookup(): SessionLookup;
	audio(): SessionAudio;
	output(): SessionOutput;
	settings(): SettingsSnapshot;
	/** Changes session state the way the engine does on its own, and tells listeners. */
	change(
		mutate: (state: {
			titles: SessionTitles;
			selection: SessionSelection;
			metadata: SessionMetadata;
			lookup: SessionLookup;
			audio: SessionAudio;
			output: SessionOutput;
			remote: import('../../types/session').RemoteUiSnapshot;
			remoteLibrary: import('../../types/session').RemoteLibrarySnapshot;
		}) => void,
	): void;
};

export function fakeRemote(): import('../../types/session').RemoteUiSnapshot {
	return {
		revision: 0,
		lane: 'audible',
		providers: [],
		account: null,
		accountStatus: { kind: 'idle' },
		auth: { kind: 'idle' },
		libraryStatus: { kind: 'idle' },
		selectedTitleIds: [],
		includePdfByTitleId: {},
		acquisition: null,
		acquiring: false,
		indexer: {
			releases: [],
			selectedReleaseKeys: [],
			releaseGrabs: {},
			searching: false,
			grabbing: false,
			message: '',
		},
		connection: {
			baseUrl: '',
			categoryIds: [3000, 3030],
			apiKeyConfigured: false,
			apiKeyEntered: false,
			save: { kind: 'idle' },
			test: { kind: 'idle' },
			testResult: null,
			draftError: null,
		},
	};
}

export function fakeOutput(): SessionOutput {
	return {
		revision: 0,
		directory: null,
		preset: 'absDefault',
		includeYear: false,
		template: '',
		naming: { preset: 'absDefault', includeYear: false, customTemplate: null },
		preview: { kind: 'noDirectory' },
		submission: null,
		restartOffers: [],
		previewRun: null,
	};
}

/** The engine's fresh audio defaults over the fixture capabilities; tests seed other choices. */
function fakeAudio(): SessionAudio {
	const settings = {
		encoderType: 'native_aac',
		bitrateKbps: 65,
		bitrateMode: { mode: 'cbr' },
		channels: 'auto',
		nativeAacSpeed: 0,
		faacProfile: 'auto',
	} as const;
	const capabilities = runtimeSettingsCapabilitiesFixture().encoder as SessionAudio['capabilities'];
	const native = capabilities?.encoderConfigurations.find(
		(config) => config.encoderType === 'native_aac',
	);
	return {
		revision: 0,
		capabilities,
		defaults: {
			choice: {
				format: 'm4b',
				intent: 'auto',
				encoder: 'native_aac',
				aacBitrateKbps: 65,
				opusBitrateKbps: 64,
				savedMode: { mode: 'cbr' },
				faacProfile: 'auto',
				faacRateControl: 'abr',
				faacQuality: 100,
				nativeSpeed: 0,
				channels: 'auto',
				sampleRate: 'auto',
			},
			facts: {
				effectiveEncoder: 'native_aac',
				encoderOptions: [
					{ encoder: 'aac_at', available: true },
					{ encoder: 'native_aac', available: true },
					{ encoder: 'faac', available: true },
				],
				encoderLocked: false,
				downmixWarning: false,
				bitrateMode: { mode: 'cbr' },
				bitrateKbpsMin: native?.bitrateKbpsMin ?? 1,
				bitrateKbpsMax: native?.bitrateKbpsMax ?? 0,
				allowedModes: native?.allowedModes ?? [],
				faacProfiles: [],
				allowedSampleRates: native?.explicitSampleRates ?? [],
				sampleRateSupported: true,
				estimateKbps: 65,
			},
			request: { format: 'm4b', intent: 'auto', settings, sampleRate: 'auto' },
		},
		titles: {},
	};
}

/** A preview of every title that finished without trouble. */
export function finishedPreview(): ProcessCommandResult {
	return {
		summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
		terminalClass: 'success',
		results: [{ inputIndex: 0, status: 'success', message: 'Preview finished', jobId: 'fake-job' }],
	};
}

function defaultSubmissionAnswer(intent: SessionIntent): SubmissionStatus {
	switch (intent.kind) {
		case 'preview':
			return { kind: 'previewFinished', result: finishedPreview() };
		case 'cancelCollisionReview':
			return { kind: 'cancelled' };
		default:
			return { kind: 'submitted', operationId: 'fake-operation', title: 'Fake title' };
	}
}

function rejection(reason: Rejection): SettingsOutcome {
	return {
		kind: 'rejected',
		error: { code: 'io_error', category: 'io', message: reason.message, detail: null },
	};
}

export function createFakeEngine(initialSettings: AppSettings = defaultAppSettings()): FakeEngine {
	let revision = 0;
	const listeners = new Set<(update: SessionUpdate) => void>();
	const settingsListeners = new Set<(snapshot: SettingsSnapshot) => void>();
	const typed = new Map<MetadataField, { value: string; blank: boolean }>();
	let boundKey = '';

	const state = {
		titles: {
			revision: 0,
			files: [],
			titleSourcesByIdentity: {},
			audioChoiceRequired: [],
			sortDirection: 'none',
			orderLocked: false,
			notice: null,
			orderDiffersFromImport: false,
			companions: {},
		} as SessionTitles,
		selection: { revision: 0, selectedIndices: [], selectedAnchor: null } as SessionSelection,
		metadata: {
			revision: 0,
			binding: 0,
			form: {
				mode: 'single',
				selectionCount: 0,
				fields: [],
				seriesPartWarning: null,
				subseriesPartWarning: null,
				validationMessage: null,
			},
			cover: {
				imageRevision: 0,
				present: false,
				custom: false,
				removalRequested: false,
				loading: false,
				notice: null,
				noticeSerial: 0,
			},
			tags: {
				title: '',
				album: '',
				artist: '',
				albumArtist: '',
				composer: '',
				series: '',
				seriesPart: '',
				subseries: '',
				subseriesPart: '',
				albumSort: '',
				year: '',
				genre: '',
			},
			saveInProgress: false,
			status: null,
			hasPendingEdits: false,
			waitingWrites: [],
		} as SessionMetadata,
		lookup: {
			revision: 0,
			open: false,
			titleQuery: '',
			authorQuery: '',
			source: 'auto',
			applyMode: 'current',
			replaceCover: false,
			status: null,
			queuePosition: null,
			results: [],
			isQueueMode: false,
			hasSearched: false,
		} as SessionLookup,
		audio: fakeAudio(),
		remote: fakeRemote(),
		remoteLibrary: { revision: 0, titles: [], diagnostics: [] },
		output: fakeOutput(),
	};

	function titleFromDefaults(): TitleAudio {
		return {
			...structuredClone(state.audio.defaults),
			plan: { kind: 'pending' },
			estimate: null,
		};
	}

	let settingsRevision = 0;
	let settingsValue: AppSettings = structuredClone(initialSettings);
	let saveError: SettingsSnapshot['saveError'];
	let concurrency = initialSettings.maxConcurrentJobs;

	const engine = {} as FakeEngine;
	let chosenCover: number[] | null = null;

	function selectedFiles(): AudioFile[] {
		return state.selection.selectedIndices
			.map((index) => state.titles.files[index])
			.filter((file): file is AudioFile => Boolean(file));
	}

	function effectiveTags(file: AudioFile): Partial<AudiobookMetadata> {
		return { ...engine.tags.get(file.path) };
	}

	/** Rebuilds what the form shows from the selected titles' tags and what was typed. */
	function project(): void {
		const selected = selectedFiles();
		const key = selected.map((file) => file.path).join('\0');
		if (key !== boundKey) {
			boundKey = key;
			typed.clear();
			chosenCover = null;
			state.metadata.status = null;
		}
		const tags = selected.filter((file) => file.isValid).map(effectiveTags);
		state.metadata.form = {
			...state.metadata.form,
			mode: selected.length > 1 ? 'multi' : 'single',
			selectionCount: selected.length > 1 ? selected.length : 0,
			fields: FIELD_TAGS.map(([field, tag]): FieldSnapshot => {
				const edit = typed.get(field);
				const values = new Set(tags.map((entry) => String(entry[tag] ?? '')));
				const shared = values.size <= 1;
				return {
					field,
					value: edit ? edit.value : shared ? ([...values][0] ?? '') : '',
					action: edit?.blank ? 'blank' : 'keep',
					dirty: Boolean(edit),
					mixed: !edit && !shared,
				};
			}),
		};
		const cover =
			chosenCover ?? (selected.length === 1 ? effectiveTags(selected[0]).cover_art : undefined);
		const present = Boolean(cover?.length);
		if (present !== state.metadata.cover.present) {
			state.metadata.cover = {
				...state.metadata.cover,
				present,
				imageRevision: state.metadata.cover.imageRevision + 1,
			};
		}
	}

	/** Stamps every part with a new revision; the fake does not track which changed. */
	function settle(): SessionUpdate {
		project();
		revision += 1;
		state.titles = { ...state.titles, revision };
		state.selection = { ...state.selection, revision };
		state.metadata = { ...state.metadata, revision };
		state.lookup = { ...state.lookup, revision };
		state.audio = { ...state.audio, revision };
		state.output = { ...state.output, revision };
		state.remote = { ...state.remote, revision };
		state.remoteLibrary = { ...state.remoteLibrary, revision };
		return structuredClone({ revision, ...state });
	}

	function publish(): void {
		const update = settle();
		for (const listener of listeners) listener(update);
	}

	function select(indices: number[]): void {
		state.selection = {
			...state.selection,
			selectedIndices: [...indices].sort((a, b) => a - b),
			selectedAnchor: indices.length ? indices[indices.length - 1] : null,
		};
	}

	function appendFiles(files: AudioFile[]): void {
		const known = new Set(state.titles.files.map((file) => file.path));
		const added = files.filter((file) => !known.has(file.path));
		const first = state.titles.files.length === 0;
		if (!first && added.length === 0) {
			state.titles.notice = { kind: 'duplicatesOnly' };
			return;
		}
		state.titles.files = [...state.titles.files, ...added];
		state.titles.notice = null;
		for (const file of added) {
			if (file.tagTitle && !engine.tags.has(file.path))
				engine.tags.set(file.path, { title: file.tagTitle, artist: file.tagArtist });
			if (file.inputId) state.audio.titles[file.inputId] = titleFromDefaults();
		}
		if (first && added.length === 1 && added[0].isValid) select([0]);
	}

	// One case per intent; each is a line or two of stand-in behavior.
	function apply(intent: SessionIntent): SessionOutcome {
		const { titles } = state;
		const locked = titles.orderLocked;
		switch (intent.kind) {
			case 'import':
				if (locked) titles.notice = { kind: 'orderLocked' };
				else appendFiles(engine.analyze(intent.paths));
				break;
			case 'importOpened': {
				const paths = engine.openedPaths.splice(0);
				if (paths.length) appendFiles(engine.analyze(paths));
				break;
			}
			case 'selectFile': {
				const current = state.selection.selectedIndices;
				const anchor = state.selection.selectedAnchor;
				if (intent.modifiers.range && anchor !== null) {
					const [from, to] = [Math.min(anchor, intent.index), Math.max(anchor, intent.index)];
					select(Array.from({ length: to - from + 1 }, (_, offset) => from + offset));
					state.selection.selectedAnchor = intent.index;
				} else if (intent.modifiers.multi) {
					select(
						current.includes(intent.index)
							? current.filter((index) => index !== intent.index)
							: [...current, intent.index],
					);
				} else select([intent.index]);
				break;
			}
			case 'selectAll':
				select(titles.files.map((_, index) => index));
				break;
			case 'clearSelection':
				select([]);
				break;
			case 'removeFile': {
				if (locked) break;
				const removedIndex = titles.files.findIndex((file) => file.inputId === intent.inputId);
				if (removedIndex < 0) break;
				titles.files = titles.files.filter((_, index) => index !== removedIndex);
				state.selection.selectedIndices = state.selection.selectedIndices
					.filter((index) => index !== removedIndex)
					.map((index) => (index > removedIndex ? index - 1 : index));
				break;
			}
			case 'clearAll':
				if (locked) break;
				titles.files = [];
				titles.titleSourcesByIdentity = {};
				state.audio.titles = {};
				state.selection.selectedIndices = [];
				break;
			// The engine's rules: a test seeds the result it needs through `change`,
			// `seedTitleAudio`, or `respond`.
			case 'moveFile':
			case 'reorderFiles':
			case 'toggleSort':
			case 'restoreImportOrder':
			case 'groupSelected':
			case 'ungroup':
			case 'reorderSources':
			case 'chooseCue':
			case 'applyDefaultAudio':
			case 'setDefaultAudio':
			case 'setTitleAudio':
			case 'loadCoverFromFile':
			case 'loadCoverFromUrl':
			case 'clearCover':
			case 'save':
			case 'lookupSearch':
			case 'lookupApply':
			case 'lookupSkip':
				break;
			case 'setOutputDirectory':
				state.output = { ...state.output, directory: intent.directory };
				break;
			case 'setNamingPreset':
				state.output = {
					...state.output,
					preset: intent.preset,
					naming: { ...state.output.naming, preset: intent.preset },
				};
				break;
			case 'setIncludeYear':
				state.output = {
					...state.output,
					includeYear: intent.includeYear,
					naming: { ...state.output.naming, includeYear: intent.includeYear },
				};
				break;
			case 'setNamingTemplate':
				state.output = { ...state.output, template: intent.template };
				break;
			case 'reset':
				titles.files = [];
				titles.titleSourcesByIdentity = {};
				state.audio.titles = {};
				titles.audioChoiceRequired = [];
				titles.orderLocked = false;
				titles.notice = null;
				state.selection.selectedIndices = [];
				state.selection.selectedAnchor = null;
				state.lookup.open = false;
				typed.clear();
				break;
			case 'setField':
				typed.set(intent.field, { value: intent.value, blank: false });
				state.metadata.status = null;
				break;
			case 'setFieldAction':
				if (intent.action === 'blank') typed.set(intent.field, { value: '', blank: true });
				else typed.delete(intent.field);
				break;
			case 'submit':
			case 'preview':
			case 'chooseCollisionPolicy':
			case 'cancelCollisionReview':
				state.output = { ...state.output, submission: engine.answerSubmission(intent) };
				break;
			case 'lookupOpen':
				state.lookup.open = true;
				break;
			case 'lookupClose':
				state.lookup.open = false;
				break;
			case 'lookupSetTitleQuery':
				state.lookup.titleQuery = intent.value;
				break;
			case 'lookupSetAuthorQuery':
				state.lookup.authorQuery = intent.value;
				break;
			case 'lookupSetSource':
				state.lookup.source = intent.source;
				break;
			case 'lookupSetApplyMode':
				state.lookup.applyMode = intent.mode;
				break;
			case 'lookupSetReplaceCover':
				state.lookup.replaceCover = intent.replace;
				break;
		}
		return { kind: 'applied' };
	}

	function settingsSnapshot(): SettingsSnapshot {
		const settings = settingsValue;
		const startup =
			settings.startupBehavior === 'pinnedDefaults' && settings.pinnedDefaults
				? settings.pinnedDefaults
				: {
						maxConcurrentJobs: settings.maxConcurrentJobs,
						encoderDefaults: settings.encoderDefaults,
						outputDefaults: settings.outputDefaults,
					};
		return structuredClone({
			revision: settingsRevision,
			settings,
			saveError,
			concurrency: {
				preference: concurrency,
				effective: concurrency.mode === 'fixed' ? concurrency.value : 4,
				capabilities: {
					allowAuto: true,
					autoEffective: 4,
					fixedMin: 1,
					fixedMax: 8,
					fixedOptions: [1, 2, 3, 4, 5, 6, 7, 8],
				},
			},
			startupDefaults: startup,
			defaultAcquisitionLane: settings.defaultAcquisitionLane,
		});
	}

	/** Puts `patch` in effect; a failed write leaves it in effect and unsaved. */
	function write(patch: Partial<AppSettings>): SettingsOutcome {
		settingsValue = { ...settingsValue, ...patch };
		const failed = engine.settingsWriteError && rejection(engine.settingsWriteError);
		saveError =
			failed && failed.kind === 'rejected'
				? { ...failed.error, detail: failed.error.detail ?? undefined }
				: undefined;
		return { kind: 'applied' };
	}

	function applySettings(intent: SettingsIntent): SettingsOutcome {
		switch (intent.kind) {
			case 'remember': {
				const { kind: _kind, ...patch } = intent;
				return write(
					Object.fromEntries(Object.entries(patch).filter(([, value]) => value !== undefined)),
				);
			}
			case 'setConcurrency':
				if (engine.concurrencyError) return rejection(engine.concurrencyError);
				concurrency = intent.preference;
				return write({ maxConcurrentJobs: intent.preference });
			case 'setKeepAwake':
				return write({ keepAwakeWhileWorking: intent.enabled });
			case 'setStartupBehavior':
				return write({ startupBehavior: intent.behavior });
			case 'pinCurrentDefaults':
				return write({
					pinnedDefaults: {
						maxConcurrentJobs: settingsValue.maxConcurrentJobs,
						encoderDefaults: settingsValue.encoderDefaults,
						outputDefaults: settingsValue.outputDefaults,
					},
				});
			case 'retry':
				return write({});
			case 'reset':
				settingsValue = defaultAppSettings();
				concurrency = { mode: 'auto' };
				return write({});
		}
	}

	let nextSessionSequence = 0;
	let nextSettingsSequence = 0;

	Object.assign(engine, {
		sessionIntents: [],
		settingsIntents: [],
		tags: new Map(),
		analyze: (paths: readonly string[]) => paths.map((path) => audioFile(path)),
		openedPaths: [],
		answerSubmission: defaultSubmissionAnswer,
		async attach() {
			nextSessionSequence = 0;
			nextSettingsSequence = 0;
			return { client: 1, session: settle(), settings: settingsSnapshot() };
		},
		async sessionDispatch(_client: number, sequence: number, intent: SessionIntent) {
			// The host applies intents in sequence order; a gap would stall it.
			if (sequence !== nextSessionSequence)
				throw new Error(`session intent ${sequence} arrived out of order`);
			nextSessionSequence += 1;
			engine.sessionIntents.push(structuredClone(intent));
			const outcome = (await engine.respond?.(intent)) ?? apply(intent);
			return { outcome, update: settle() };
		},
		async settingsDispatch(_client: number, sequence: number, intent: SettingsIntent) {
			if (sequence !== nextSettingsSequence)
				throw new Error(`settings intent ${sequence} arrived out of order`);
			nextSettingsSequence += 1;
			engine.settingsIntents.push(structuredClone(intent));
			const outcome = applySettings(intent);
			settingsRevision += 1;
			return { outcome, snapshot: settingsSnapshot() };
		},
		async sessionCoverArt() {
			if (chosenCover) return chosenCover;
			const [file] = selectedFiles();
			return file ? (effectiveTags(file).cover_art ?? null) : null;
		},
		seedGroup(sources: AudioFile[], options = {}) {
			const [anchor] = sources;
			const id = anchor?.inputId;
			if (!id) throw new Error('a group needs an anchor with an input id');
			const members = new Set(sources.map((file) => file.path));
			const titles = state.titles;
			titles.files = titles.files.filter(
				(file) => file.path === anchor.path || !members.has(file.path),
			);
			titles.titleSourcesByIdentity = { ...titles.titleSourcesByIdentity, [id]: sources };
			if (options.choiceRequired) titles.audioChoiceRequired = [...titles.audioChoiceRequired, id];
			select([titles.files.findIndex((file) => file.path === anchor.path)]);
			publish();
		},
		seedField(field: MetadataField, value: string) {
			typed.set(field, { value, blank: false });
			publish();
		},
		seedCover(bytes: number[], notice?: CoverNotice) {
			chosenCover = bytes;
			const cover = state.metadata.cover;
			state.metadata.cover = {
				...cover,
				present: true,
				custom: true,
				imageRevision: cover.imageRevision + 1,
				notice: notice ?? cover.notice,
				noticeSerial: notice ? cover.noticeSerial + 1 : cover.noticeSerial,
			};
			publish();
		},
		async listenSessionUpdates(handler: (update: SessionUpdate) => void) {
			listeners.add(handler);
			return () => listeners.delete(handler);
		},
		async listenSettingsUpdates(handler: (snapshot: SettingsSnapshot) => void) {
			settingsListeners.add(handler);
			return () => settingsListeners.delete(handler);
		},
		recordSettings(patch: Partial<AppSettings>) {
			write(patch);
			settingsRevision += 1;
			const snapshot = settingsSnapshot();
			for (const listener of settingsListeners) listener(snapshot);
		},
		loadTitles(files: AudioFile[], selected: number[] = []) {
			appendFiles(files);
			if (selected.length) select(selected);
			publish();
		},
		status(status: MetadataStatus | null) {
			state.metadata.status = status;
			publish();
		},
		titles: () => state.titles,
		selection: () => state.selection,
		metadata: () => {
			project();
			return state.metadata;
		},
		lookup: () => state.lookup,
		audio: () => state.audio,
		output: () => state.output,
		seedTitleAudio(titleId, request, resolved = {}) {
			const view = structuredClone(state.audio.titles[titleId] ?? titleFromDefaults());
			view.request = structuredClone(request);
			view.choice = { ...view.choice, format: request.format, intent: request.intent };
			if (resolved.plan) view.plan = resolved.plan;
			if (resolved.estimate !== undefined) view.estimate = resolved.estimate;
			state.audio.titles = { ...state.audio.titles, [titleId]: view };
			// A title given its own request no longer needs a choice.
			state.titles.audioChoiceRequired = state.titles.audioChoiceRequired.filter(
				(id) => id !== titleId,
			);
			publish();
		},
		settings: settingsSnapshot,
		change(mutate: Parameters<FakeEngine['change']>[0]) {
			mutate(state);
			publish();
		},
	} satisfies Partial<FakeEngine>);

	return engine;
}

let current = createFakeEngine();

/** The fake engine the mocked backend answers from in this test. */
export function fakeEngine(): FakeEngine {
	return current;
}

export function resetFakeEngine(): void {
	current = createFakeEngine();
}
