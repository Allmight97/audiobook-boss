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
import type { AudioFile, TitleAudioRequest } from '../../types/audio';
import type { AudiobookMetadata, OnlineMetadataResult } from '../../types/metadata';
import type { MetadataFieldIntent, MetadataIntentPatch } from '../../types/metadataIntent';
import type {
	FieldSnapshot,
	MetadataField,
	MetadataStatus,
	SessionIntent,
	SessionLookup,
	SessionMetadata,
	SessionOutcome,
	SessionSelection,
	SessionTitles,
	SessionUpdate,
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
	/** The pending edits each Save wrote, by file path. */
	readonly saves: Array<Record<string, MetadataIntentPatch>>;
	/** What reading each file's tags yields. */
	readonly tags: Map<string, Partial<AudiobookMetadata>>;
	/** What importing a set of paths yields. Defaults to one valid file per path. */
	analyze: (paths: readonly string[]) => AudioFile[];
	/** Paths the operating system asked the app to open. */
	openedPaths: string[];
	lookupResults: OnlineMetadataResult[];
	/** Makes every lookup search fail, as unreachable providers would. */
	searchFails: boolean;
	coverBytes: number[];
	/** Answers an intent in place of the fake's default behavior. */
	respond?: (intent: SessionIntent) => SessionOutcome | Promise<SessionOutcome> | undefined;
	/** Called with the files a Save wrote. */
	afterSave?: (filePaths: string[]) => void;
	/** Rejects the next settings writes, as a full disk would. */
	settingsWriteError?: Rejection;
	/** Refuses the next concurrency change, as running jobs would. */
	concurrencyError?: Rejection;
	/** Refuses recovery, as an unwritable backup would. */
	recoverError?: Rejection;
	/** Makes the saved settings unreadable until recovered. */
	breakSettings(recovery?: SettingsSnapshot['recovery']): void;
	/** Loads titles as an import would. */
	loadTitles(files: AudioFile[], selected?: number[]): void;
	pendingEdits(path: string): MetadataIntentPatch | undefined;
	status(status: MetadataStatus | null): void;
	titles(): SessionTitles;
	selection(): SessionSelection;
	metadata(): SessionMetadata;
	lookup(): SessionLookup;
	settings(): SettingsSnapshot;
	/** Changes session state the way the engine does on its own, and tells listeners. */
	change(
		mutate: (state: {
			titles: SessionTitles;
			selection: SessionSelection;
			metadata: SessionMetadata;
			lookup: SessionLookup;
		}) => void,
	): void;
};

function rejection(reason: Rejection): SettingsOutcome {
	return {
		kind: 'rejected',
		error: { code: 'io_error', category: 'io', message: reason.message, detail: null },
	};
}

export function createFakeEngine(initialSettings: AppSettings = defaultAppSettings()): FakeEngine {
	let revision = 0;
	const listeners = new Set<(update: SessionUpdate) => void>();
	const pending = new Map<string, MetadataIntentPatch>();
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
			audioRequestsByIdentity: {},
		} as SessionTitles,
		selection: { revision: 0, selectedIndices: [], selectedAnchor: null } as SessionSelection,
		metadata: {
			revision: 0,
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
			albumSort: null,
			saveInProgress: false,
			status: null,
			hasPendingEdits: false,
			deferredWrites: [],
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
	};

	let settingsRevision = 0;
	let settingsValue: AppSettings | undefined = structuredClone(initialSettings);
	let unsaved: Partial<AppSettings> = {};
	let saveError: SettingsSnapshot['saveError'];
	let loadError: SettingsSnapshot['loadError'];
	let recovery: SettingsSnapshot['recovery'];
	let concurrency = initialSettings.maxConcurrentJobs;
	let lookupQueue: AudioFile[] = [];

	const engine = {} as FakeEngine;

	function selectedFiles(): AudioFile[] {
		return state.selection.selectedIndices
			.map((index) => state.titles.files[index])
			.filter((file): file is AudioFile => Boolean(file));
	}

	function effectiveTags(file: AudioFile): Partial<AudiobookMetadata> {
		const tags: Record<string, unknown> = { ...engine.tags.get(file.path) };
		for (const [key, intent] of Object.entries(pending.get(file.path) ?? {})) {
			const op = intent as MetadataFieldIntent;
			if (op.op === 'set') tags[key] = op.value;
			else if (op.op === 'clear') delete tags[key];
		}
		return tags as Partial<AudiobookMetadata>;
	}

	/** Rebuilds what the form shows from the selected titles' tags and what was typed. */
	function project(): void {
		const selected = selectedFiles();
		const key = selected.map((file) => file.path).join('\0');
		if (key !== boundKey) {
			boundKey = key;
			typed.clear();
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
		const cover = selected.length === 1 ? effectiveTags(selected[0]).cover_art : undefined;
		const present = Boolean(cover?.length);
		if (present !== state.metadata.cover.present) {
			state.metadata.cover = {
				...state.metadata.cover,
				present,
				imageRevision: state.metadata.cover.imageRevision + 1,
			};
		}
		state.metadata.hasPendingEdits = pending.size > 0;
	}

	/** Stamps every part with a new revision; the fake does not track which changed. */
	function settle(): SessionUpdate {
		project();
		revision += 1;
		state.titles = { ...state.titles, revision };
		state.selection = { ...state.selection, revision };
		state.metadata = { ...state.metadata, revision };
		state.lookup = { ...state.lookup, revision };
		return structuredClone({ revision, ...state });
	}

	function publish(): void {
		const update = settle();
		for (const listener of listeners) listener(update);
	}

	function stage(): void {
		const patch: Record<string, MetadataFieldIntent> = {};
		for (const [field, tag] of FIELD_TAGS) {
			const edit = typed.get(field);
			if (!edit) continue;
			const value = edit.blank ? '' : edit.value.trim();
			const op: MetadataFieldIntent = value ? { op: 'set', value } : { op: 'clear' };
			patch[tag] = op;
			if (field === 'title') patch.album = op;
		}
		if (Object.keys(patch).length === 0) return;
		for (const file of selectedFiles().filter((file) => file.isValid)) {
			pending.set(file.path, { ...pending.get(file.path), ...patch } as MetadataIntentPatch);
		}
		typed.clear();
	}

	function select(indices: number[]): void {
		stage();
		state.selection = {
			...state.selection,
			selectedIndices: [...indices].sort((a, b) => a - b),
			selectedAnchor: indices.length ? indices[indices.length - 1] : null,
		};
	}

	function appendFiles(files: AudioFile[], defaultAudio: TitleAudioRequest | undefined): void {
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
			if (defaultAudio && file.inputId)
				state.titles.audioRequestsByIdentity[file.inputId] = structuredClone(defaultAudio);
		}
		if (first && added.length === 1 && added[0].isValid) select([0]);
	}

	function runSearch(after: 'applied' | 'skipped' | null): void {
		if (engine.searchFails) {
			state.lookup.results = [];
			state.lookup.hasSearched = false;
			state.lookup.status = { kind: 'searchFailed', after };
			return;
		}
		state.lookup.results = structuredClone(engine.lookupResults);
		state.lookup.hasSearched = true;
		state.lookup.status = {
			kind: 'found',
			count: state.lookup.results.length,
			partial: false,
			after,
		};
	}

	function showQueued(index: number): void {
		const file = lookupQueue[index];
		state.lookup.queuePosition = file
			? { index, total: lookupQueue.length, path: file.path }
			: null;
		state.lookup.titleQuery = file ? String(effectiveTags(file).title ?? '') : '';
		state.lookup.authorQuery = file ? String(effectiveTags(file).artist ?? '') : '';
	}

	function advanceLookup(step: 'applied' | 'skipped'): void {
		const next = (state.lookup.queuePosition?.index ?? 0) + 1;
		const file = lookupQueue[next];
		if (!file) {
			state.lookup.status = { kind: 'queueComplete', coverFailed: false };
			return;
		}
		select([state.titles.files.indexOf(file)]);
		showQueued(next);
		runSearch(step);
	}

	function applyLookup(index: number): void {
		const result = state.lookup.results[index];
		const position = state.lookup.queuePosition;
		if (!result || !position) return;
		select([state.titles.files.indexOf(lookupQueue[position.index])]);
		const values: Array<[MetadataField, string | undefined]> = [
			['title', result.title],
			['author', result.authors.join(', ') || undefined],
			['narrator', result.narrators.join(', ') || undefined],
			['series', result.series],
			['seriesPart', result.seriesPart],
			['subseries', result.subseries],
			['subseriesPart', result.subseriesPart],
			['description', result.description],
			['date', result.publishedDate],
		];
		for (const [field, value] of values) {
			if (value !== undefined) typed.set(field, { value, blank: false });
		}
		if (state.lookup.replaceCover && result.coverUrl) setCover(engine.coverBytes);
		if (state.lookup.applyMode === 'queue') advanceLookup('applied');
		else state.lookup.status = { kind: 'applied', coverFailed: false };
	}

	function setCover(bytes: number[] | null): void {
		const [file] = selectedFiles();
		if (!file) return;
		pending.set(file.path, {
			...pending.get(file.path),
			cover_art: bytes ? { op: 'set', value: bytes } : { op: 'clear' },
		});
		state.metadata.cover = {
			...state.metadata.cover,
			custom: Boolean(bytes),
			removalRequested: !bytes,
			imageRevision: state.metadata.cover.imageRevision + 1,
		};
	}

	function swap(index: number, target: number): void {
		const files = [...state.titles.files];
		if (target < 0 || target >= files.length) return;
		const selected = selectedFiles();
		[files[index], files[target]] = [files[target], files[index]];
		state.titles.files = files;
		state.selection.selectedIndices = selected
			.map((file) => files.indexOf(file))
			.sort((a, b) => a - b);
	}

	// One case per intent; each is a line or two of stand-in behavior.
	function apply(intent: SessionIntent): SessionOutcome {
		const { titles } = state;
		const locked = titles.orderLocked;
		switch (intent.kind) {
			case 'import':
				if (locked) titles.notice = { kind: 'orderLocked' };
				else appendFiles(engine.analyze(intent.paths), intent.defaultAudio);
				break;
			case 'importOpened': {
				const paths = engine.openedPaths.splice(0);
				if (paths.length) appendFiles(engine.analyze(paths), intent.defaultAudio);
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
			case 'removeFile':
				if (locked) break;
				stage();
				titles.files = titles.files.filter((_, index) => index !== intent.index);
				state.selection.selectedIndices = state.selection.selectedIndices
					.filter((index) => index !== intent.index)
					.map((index) => (index > intent.index ? index - 1 : index));
				break;
			case 'clearAll':
				if (locked) break;
				titles.files = [];
				titles.titleSourcesByIdentity = {};
				titles.audioRequestsByIdentity = {};
				state.selection.selectedIndices = [];
				pending.clear();
				break;
			case 'moveFile':
				if (!locked) swap(intent.index, intent.index + (intent.direction === 'up' ? -1 : 1));
				break;
			case 'reorderFiles': {
				if (locked) break;
				const selected = selectedFiles();
				const files = [...titles.files];
				const [moved] = files.splice(intent.from, 1);
				if (moved) files.splice(intent.to, 0, moved);
				titles.files = files;
				state.selection.selectedIndices = selected
					.map((file) => files.indexOf(file))
					.sort((a, b) => a - b);
				break;
			}
			case 'toggleSort': {
				if (locked) break;
				const selected = selectedFiles();
				const descending = titles.sortDirection === 'ascending';
				titles.files = [...titles.files].sort(
					(a, b) =>
						a.path.localeCompare(b.path, undefined, { numeric: true }) * (descending ? -1 : 1),
				);
				titles.sortDirection = descending ? 'descending' : 'ascending';
				state.selection.selectedIndices = selected
					.map((file) => titles.files.indexOf(file))
					.sort((a, b) => a - b);
				break;
			}
			case 'restoreImportOrder':
				titles.sortDirection = 'none';
				break;
			case 'setOrderLocked':
				titles.orderLocked = intent.locked;
				break;
			case 'groupSelected': {
				const selected = selectedFiles();
				const [anchor] = selected;
				if (locked || selected.length < 2 || !anchor.inputId) break;
				const requests = selected.map((file) =>
					JSON.stringify(titles.audioRequestsByIdentity[file.inputId ?? ''] ?? null),
				);
				titles.titleSourcesByIdentity[anchor.inputId] = selected.flatMap(
					(file) => titles.titleSourcesByIdentity[file.inputId ?? ''] ?? [file],
				);
				titles.files = titles.files.filter((file) => file === anchor || !selected.includes(file));
				if (new Set(requests).size > 1) titles.audioChoiceRequired.push(anchor.inputId);
				select([titles.files.indexOf(anchor)]);
				break;
			}
			case 'ungroup': {
				const sources = titles.titleSourcesByIdentity[intent.titleId];
				const index = titles.files.findIndex((file) => file.inputId === intent.titleId);
				if (locked || !sources || index < 0) break;
				delete titles.titleSourcesByIdentity[intent.titleId];
				titles.audioChoiceRequired = titles.audioChoiceRequired.filter(
					(id) => id !== intent.titleId,
				);
				titles.files = [
					...titles.files.slice(0, index),
					...sources,
					...titles.files.slice(index + 1),
				];
				select(sources.map((_, offset) => index + offset));
				break;
			}
			case 'reorderSources': {
				const sources = [...(titles.titleSourcesByIdentity[intent.titleId] ?? [])];
				const [moved] = sources.splice(intent.from, 1);
				if (moved) sources.splice(intent.to, 0, moved);
				if (sources.length) titles.titleSourcesByIdentity[intent.titleId] = sources;
				break;
			}
			case 'chooseCue':
				titles.files = titles.files.map((file) =>
					file.inputId === intent.inputId && file.cueSource
						? {
								...file,
								cueSource: {
									...file.cueSource,
									status: intent.choice === 'ignore' ? 'ignored' : 'ready',
								},
							}
						: file,
				);
				break;
			case 'setAudioRequest':
				if (locked) break;
				titles.audioRequestsByIdentity[intent.titleId] = intent.request;
				titles.audioChoiceRequired = titles.audioChoiceRequired.filter(
					(id) => id !== intent.titleId,
				);
				break;
			case 'reset':
				titles.files = [];
				titles.titleSourcesByIdentity = {};
				titles.audioRequestsByIdentity = {};
				titles.audioChoiceRequired = [];
				titles.orderLocked = false;
				titles.notice = null;
				state.selection.selectedIndices = [];
				state.selection.selectedAnchor = null;
				state.lookup.open = false;
				pending.clear();
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
			case 'loadCoverFromFile':
			case 'loadCoverFromUrl':
				setCover(engine.coverBytes);
				if (intent.kind === 'loadCoverFromUrl') {
					state.metadata.cover.notice = { kind: 'loadedFromUrl' };
					state.metadata.cover.noticeSerial += 1;
				}
				break;
			case 'clearCover':
				setCover(null);
				break;
			case 'stageSelection':
				stage();
				break;
			case 'save': {
				stage();
				const written = Object.fromEntries(pending);
				const count = Object.keys(written).length;
				if (count > 0) {
					engine.saves.push(written);
					engine.afterSave?.(Object.keys(written));
				}
				for (const [path, patch] of pending) {
					const tags: Record<string, unknown> = { ...engine.tags.get(path) };
					for (const [key, op] of Object.entries(patch as Record<string, MetadataFieldIntent>)) {
						if (op.op === 'set') tags[key] = op.value;
						else if (op.op === 'clear') delete tags[key];
					}
					engine.tags.set(path, tags as Partial<AudiobookMetadata>);
				}
				pending.clear();
				state.metadata.cover = { ...state.metadata.cover, custom: false, removalRequested: false };
				state.metadata.status =
					count > 0
						? {
								kind: 'saveComplete',
								succeeded: count,
								failed: 0,
								cancelled: 0,
								waiting: 0,
								held: 0,
							}
						: { kind: 'noPendingChanges' };
				break;
			}
			case 'lookupOpen':
				lookupQueue = selectedFiles().filter((file) => file.isValid);
				state.lookup.open = true;
				state.lookup.isQueueMode = lookupQueue.length > 1;
				state.lookup.applyMode = lookupQueue.length > 1 ? 'queue' : 'current';
				state.lookup.replaceCover = false;
				state.lookup.results = [];
				state.lookup.hasSearched = false;
				showQueued(0);
				if (lookupQueue.length === 0) state.lookup.status = { kind: 'noValidTitle' };
				else runSearch(null);
				break;
			case 'lookupClose':
				state.lookup.open = false;
				break;
			case 'lookupSearch':
				if (!`${state.lookup.titleQuery}${state.lookup.authorQuery}`.trim())
					state.lookup.status = { kind: 'queryRequired' };
				else runSearch(null);
				break;
			case 'lookupApply':
				applyLookup(intent.index);
				break;
			case 'lookupSkip':
				advanceLookup('skipped');
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
		const settings = settingsValue ? { ...settingsValue, ...unsaved } : undefined;
		const startup =
			settings?.startupBehavior === 'pinnedDefaults' && settings.pinnedDefaults
				? settings.pinnedDefaults
				: settings && {
						maxConcurrentJobs: settings.maxConcurrentJobs,
						encoderDefaults: settings.encoderDefaults,
						outputDefaults: settings.outputDefaults,
					};
		return structuredClone({
			revision: settingsRevision,
			settings,
			loadError,
			recovery,
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
			defaultAcquisitionLane:
				unsaved.defaultAcquisitionLane ?? settings?.defaultAcquisitionLane ?? 'audible',
		});
	}

	function write(patch: Partial<AppSettings>, mustSave: boolean): SettingsOutcome {
		if (engine.settingsWriteError) {
			const refused = rejection(engine.settingsWriteError);
			if (mustSave) return refused;
			unsaved = { ...unsaved, ...patch };
			saveError =
				refused.kind === 'rejected'
					? { ...refused.error, detail: refused.error.detail ?? undefined }
					: undefined;
			return { kind: 'applied' };
		}
		if (!settingsValue) {
			unsaved = { ...unsaved, ...patch };
			saveError = loadError;
			return mustSave && loadError
				? { kind: 'rejected', error: { ...loadError, detail: loadError.detail ?? null } }
				: { kind: 'applied' };
		}
		settingsValue = { ...settingsValue, ...unsaved, ...patch };
		unsaved = {};
		saveError = undefined;
		return { kind: 'applied' };
	}

	function applySettings(intent: SettingsIntent): SettingsOutcome {
		switch (intent.kind) {
			case 'remember': {
				const { kind: _kind, ...patch } = intent;
				return write(
					Object.fromEntries(Object.entries(patch).filter(([, value]) => value !== undefined)),
					false,
				);
			}
			case 'setConcurrency':
				if (engine.concurrencyError) return rejection(engine.concurrencyError);
				concurrency = intent.preference;
				return write({ maxConcurrentJobs: intent.preference }, false);
			case 'setKeepAwake':
				return write({ keepAwakeWhileWorking: intent.enabled }, true);
			case 'setStartupBehavior':
				return write({ startupBehavior: intent.behavior }, true);
			case 'pinCurrentDefaults': {
				const saved = write({}, true);
				if (saved.kind !== 'applied' || !settingsValue) return saved;
				return write(
					{
						pinnedDefaults: {
							maxConcurrentJobs: settingsValue.maxConcurrentJobs,
							encoderDefaults: settingsValue.encoderDefaults,
							outputDefaults: settingsValue.outputDefaults,
						},
					},
					true,
				);
			}
			case 'retry':
				return write({}, false);
			case 'reset':
				if (engine.settingsWriteError) return rejection(engine.settingsWriteError);
				settingsValue = defaultAppSettings();
				unsaved = {};
				saveError = undefined;
				loadError = undefined;
				recovery = undefined;
				concurrency = { mode: 'auto' };
				return { kind: 'applied' };
			case 'recover':
				if (engine.recoverError) return rejection(engine.recoverError);
				settingsValue = defaultAppSettings();
				loadError = undefined;
				recovery = undefined;
				write({}, false);
				return { kind: 'recovered', backupFileName: 'app-settings.before-recovery.json' };
			case 'reload':
				return { kind: 'applied' };
		}
	}

	let nextSessionSequence = 0;
	let nextSettingsSequence = 0;

	Object.assign(engine, {
		sessionIntents: [],
		settingsIntents: [],
		saves: [],
		tags: new Map(),
		analyze: (paths: readonly string[]) => paths.map((path) => audioFile(path)),
		openedPaths: [],
		lookupResults: [],
		searchFails: false,
		coverBytes: [1, 2, 3],
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
			const [file] = selectedFiles();
			return file ? (effectiveTags(file).cover_art ?? null) : null;
		},
		async sessionMetadataIntents(filePaths: string[]) {
			return Object.fromEntries(
				filePaths.flatMap((path) => {
					const patch = pending.get(path);
					return patch ? [[path, structuredClone(patch)]] : [];
				}),
			);
		},
		async listenSessionUpdates(handler: (update: SessionUpdate) => void) {
			listeners.add(handler);
			return () => listeners.delete(handler);
		},
		breakSettings(plan: SettingsSnapshot['recovery']) {
			settingsValue = undefined;
			recovery = plan;
			loadError = {
				code: 'invalid_input',
				category: 'validation',
				message: 'App settings file could not be read by this version.',
			};
			settingsRevision += 1;
		},
		loadTitles(files: AudioFile[], selected: number[] = []) {
			appendFiles(files, undefined);
			if (selected.length) select(selected);
			publish();
		},
		pendingEdits: (path: string) => pending.get(path),
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
