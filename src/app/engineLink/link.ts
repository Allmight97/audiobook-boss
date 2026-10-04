import { createSignal, type Accessor } from 'solid-js';
import { liveEngineCapability, type EngineCapability } from '../../lib/tauri/capabilities/engine';
import type { SettingsIntent, SettingsOutcome, SettingsSnapshot } from '../../types/appSettings';
import type { AudioFile } from '../../types/audio';
import type {
	RemoteLibrarySnapshot,
	SessionIntent,
	SessionAudio,
	SessionLookup,
	SessionOutput,
	SessionMetadata,
	SessionOutcome,
	SessionSelection,
	SessionTitles,
	SessionUpdate,
} from '../../types/session';

/**
 * The frontend's connection to the engine: the latest session and settings
 * the engine reported, and the way to send it intents.
 */
export type EngineLink = {
	readonly titles: Accessor<SessionTitles>;
	readonly selection: Accessor<SessionSelection>;
	readonly metadata: Accessor<SessionMetadata>;
	readonly lookup: Accessor<SessionLookup>;
	readonly audio: Accessor<SessionAudio>;
	readonly output: Accessor<SessionOutput>;
	readonly remote: Accessor<import('../../types/session').RemoteUiSnapshot>;
	readonly remoteLibrary: Accessor<RemoteLibrarySnapshot>;
	readonly settings: Accessor<SettingsSnapshot>;
	/** Sends an intent and resolves with its outcome once its work has finished. */
	send(intent: SessionIntent): Promise<SessionOutcome>;
	/** Sends an intent whose outcome the caller does not need; a failure is logged. */
	post(intent: SessionIntent): void;
	sendSettings(intent: SettingsIntent): Promise<SettingsOutcome>;
	coverArt(): Promise<number[] | null>;
	/** Resolves once the engine's current state has been received. */
	ready(): Promise<void>;
	dispose(): void;
};

const UNATTACHED = -1;

function emptyTitles(): SessionTitles {
	return {
		revision: UNATTACHED,
		files: [],
		titleSourcesByIdentity: {},
		audioChoiceRequired: [],
		sortDirection: 'none',
		orderLocked: false,
		notice: null,
		orderDiffersFromImport: false,
		companions: {},
	};
}

function emptyMetadata(): SessionMetadata {
	return {
		revision: UNATTACHED,
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
	};
}

function emptyLookup(): SessionLookup {
	return {
		revision: UNATTACHED,
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
	};
}

function emptyOutput(): SessionOutput {
	return {
		revision: UNATTACHED,
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

/** What shows before the engine's audio part arrives: the fresh defaults, no capabilities. */
function emptyAudio(): SessionAudio {
	const settings = {
		encoderType: 'native_aac',
		bitrateKbps: 65,
		bitrateMode: { mode: 'cbr' },
		channels: 'auto',
		nativeAacSpeed: 0,
		faacProfile: 'auto',
	} as const;
	return {
		revision: UNATTACHED,
		capabilities: null,
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
				encoderOptions: [],
				encoderLocked: true,
				downmixWarning: false,
				bitrateMode: { mode: 'cbr' },
				bitrateKbpsMin: 1,
				bitrateKbpsMax: 0,
				allowedModes: [],
				faacProfiles: [],
				allowedSampleRates: [],
				sampleRateSupported: true,
				estimateKbps: 65,
			},
			request: { format: 'm4b', intent: 'auto', settings, sampleRate: 'auto' },
		},
		titles: {},
	};
}

function emptyRemote(): import('../../types/session').RemoteUiSnapshot {
	return {
		revision: UNATTACHED,
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
			categoryIds: [],
			apiKeyConfigured: false,
			apiKeyEntered: false,
			save: { kind: 'idle' },
			test: { kind: 'idle' },
			testResult: null,
			draftError: null,
		},
	};
}

function emptySettings(): SettingsSnapshot {
	return {
		revision: UNATTACHED,
		concurrency: {
			preference: { mode: 'auto' },
			effective: 1,
			capabilities: {
				allowAuto: true,
				autoEffective: 1,
				fixedMin: 1,
				fixedMax: 1,
				fixedOptions: [],
			},
		},
		defaultAcquisitionLane: 'audible',
	};
}

/**
 * Keeps the existing object for every file whose content did not change.
 * Views key their rows by file object, so an update that changes one title
 * must not rebuild the others.
 */
function shareUnchangedFiles(previous: SessionTitles, next: SessionTitles): SessionTitles {
	const known = new Map<string, { file: AudioFile; content: string }>();
	const remember = (file: AudioFile) =>
		known.set(file.inputId ?? file.path, { file, content: JSON.stringify(file) });
	previous.files.forEach(remember);
	Object.values(previous.titleSourcesByIdentity).flat().forEach(remember);
	const share = (file: AudioFile): AudioFile => {
		const existing = known.get(file.inputId ?? file.path);
		return existing && existing.content === JSON.stringify(file) ? existing.file : file;
	};
	return {
		...next,
		files: next.files.map(share),
		titleSourcesByIdentity: Object.fromEntries(
			Object.entries(next.titleSourcesByIdentity).map(([id, sources]) => [id, sources.map(share)]),
		),
	};
}

/** The same for lookup results: an unrelated lookup change keeps their rows. */
function shareUnchangedResults(previous: SessionLookup, next: SessionLookup): SessionLookup {
	const known = new Map(previous.results.map((result) => [JSON.stringify(result), result]));
	return {
		...next,
		results: next.results.map((result) => known.get(JSON.stringify(result)) ?? result),
	};
}

export function createEngineLink(capability: EngineCapability = liveEngineCapability): EngineLink {
	let titles = emptyTitles();
	let selection: SessionSelection = {
		revision: UNATTACHED,
		selectedIndices: [],
		selectedAnchor: null,
	};
	let metadata = emptyMetadata();
	let lookup = emptyLookup();
	let audio = emptyAudio();
	let output = emptyOutput();
	let remote = emptyRemote();
	let remoteLibrary: RemoteLibrarySnapshot = { revision: UNATTACHED, titles: [], diagnostics: [] };
	let settings = emptySettings();
	const part = () => createSignal(0, { ownedWrite: true });
	const [titlesRev, bumpTitles] = part();
	const [selectionRev, bumpSelection] = part();
	const [metadataRev, bumpMetadata] = part();
	const [lookupRev, bumpLookup] = part();
	const [audioRev, bumpAudio] = part();
	const [outputRev, bumpOutput] = part();
	const [remoteRev, bumpRemote] = part();
	const [libraryRev, bumpLibrary] = part();
	const [settingsRev, bumpSettings] = part();
	let disposed = false;
	let sessionSequence = 0;
	let settingsSequence = 0;
	let unlisten: (() => void) | undefined;

	/** Keeps whichever copy of each part is newest; replies and events may cross. */
	function applySession(update: SessionUpdate): void {
		if (disposed) return;
		if (update.titles && update.titles.revision > titles.revision) {
			titles = shareUnchangedFiles(titles, update.titles);
			bumpTitles((n) => n + 1);
		}
		if (update.selection && update.selection.revision > selection.revision) {
			selection = update.selection;
			bumpSelection((n) => n + 1);
		}
		if (update.metadata && update.metadata.revision > metadata.revision) {
			metadata = update.metadata;
			bumpMetadata((n) => n + 1);
		}
		if (update.remote && update.remote.revision > remote.revision) {
			remote = update.remote;
			bumpRemote((n) => n + 1);
		}
		if (update.remoteLibrary && update.remoteLibrary.revision > remoteLibrary.revision) {
			remoteLibrary = update.remoteLibrary;
			bumpLibrary((n) => n + 1);
		}
		if (update.output && update.output.revision > output.revision) {
			output = update.output;
			bumpOutput((n) => n + 1);
		}
		if (update.audio && update.audio.revision > audio.revision) {
			audio = update.audio;
			bumpAudio((n) => n + 1);
		}
		if (update.lookup && update.lookup.revision > lookup.revision) {
			lookup = shareUnchangedResults(lookup, update.lookup);
			bumpLookup((n) => n + 1);
		}
	}

	function applySettings(snapshot: SettingsSnapshot): void {
		if (snapshot.revision <= settings.revision) return;
		settings = snapshot;
		// A reply that lands after disposal is still the truth for whoever
		// awaited it; only the views are gone.
		if (!disposed) bumpSettings((n) => n + 1);
	}

	// Updates are heard before the attach reply, so nothing published in
	// between is missed; part revisions make applying one twice harmless.
	const attached = Promise.all([
		capability.listenSessionUpdates(applySession),
		capability.listenSettingsUpdates(applySettings),
	])
		.then((stops) => {
			const stop = () => {
				for (const each of stops) each();
			};
			if (disposed) {
				stop();
				throw new Error('Engine link was disposed before attachment.');
			}
			unlisten = stop;
			return capability.attach();
		})
		.then((attachment) => {
			applySession(attachment.session);
			applySettings(attachment.settings);
			return attachment.client;
		});
	// A failed attach surfaces through the first send; it must not also be an
	// unhandled rejection.
	attached.catch(() => undefined);

	function send(intent: SessionIntent): Promise<SessionOutcome> {
		// The sequence is taken as the request is made, so intents are numbered
		// in the order they were sent.
		return attached
			.then((client) => capability.sessionDispatch(client, sessionSequence++, intent))
			.then((reply) => {
				applySession(reply.update);
				if (reply.outcome.kind === 'rejected') throw reply.outcome.error;
				return reply.outcome;
			});
	}

	return {
		titles: () => {
			titlesRev();
			return titles;
		},
		selection: () => {
			selectionRev();
			return selection;
		},
		metadata: () => {
			metadataRev();
			return metadata;
		},
		audio: () => {
			audioRev();
			return audio;
		},
		remote: () => {
			remoteRev();
			return remote;
		},
		remoteLibrary: () => {
			libraryRev();
			return remoteLibrary;
		},
		output: () => {
			outputRev();
			return output;
		},
		lookup: () => {
			lookupRev();
			return lookup;
		},
		settings: () => {
			settingsRev();
			return settings;
		},
		send,
		post(intent) {
			send(intent).catch((error: unknown) => {
				if (!disposed) console.error(`Session intent ${intent.kind} failed:`, error);
			});
		},
		sendSettings(intent) {
			return attached
				.then((client) => capability.settingsDispatch(client, settingsSequence++, intent))
				.then((reply) => {
					applySettings(reply.snapshot);
					return reply.outcome;
				});
		},
		coverArt: () => capability.sessionCoverArt(),
		ready: () => attached.then(() => undefined),
		dispose() {
			disposed = true;
			unlisten?.();
			unlisten = undefined;
		},
	};
}
