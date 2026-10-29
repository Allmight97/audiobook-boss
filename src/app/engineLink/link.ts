import { createSignal, type Accessor } from 'solid-js';
import { isCancellation, toUserMessage } from '../../lib/tauri/appError';
import { liveEngineCapability, type EngineCapability } from '../../lib/tauri/capabilities/engine';
import type { SettingsIntent, SettingsOutcome, SettingsSnapshot } from '../../types/appSettings';
import type { AudioFile } from '../../types/audio';
import type {
	RemoteAcquisitionSnapshot,
	RemoteLibrarySnapshot,
	RemoteUiSnapshot,
	SessionIntent,
	SessionAudio,
	SessionLookup,
	SessionOutput,
	SessionMetadata,
	SessionOutcome,
	SessionReply,
	SessionSelection,
	SessionTitles,
	SessionUpdate,
} from '../../types/session';

export type EngineLink = {
	readonly titles: Accessor<SessionTitles>;
	readonly selection: Accessor<SessionSelection>;
	readonly metadata: Accessor<SessionMetadata>;
	readonly lookup: Accessor<SessionLookup>;
	readonly audio: Accessor<SessionAudio>;
	readonly output: Accessor<SessionOutput>;
	readonly remote: Accessor<RemoteUiSnapshot>;
	readonly remoteLibrary: Accessor<RemoteLibrarySnapshot>;
	readonly remoteAcquisition: Accessor<RemoteAcquisitionSnapshot>;
	readonly settings: Accessor<SettingsSnapshot>;
	/** Sends an intent and resolves with its outcome once its work has finished. */
	send(intent: SessionIntent): Promise<SessionOutcome>;
	openPreview(runId: string): Promise<SessionOutcome>;
	/** Sends an intent whose outcome the caller does not need. A refusal is
	 * kept as `refusal` until dismissed. */
	post(intent: SessionIntent): void;
	/** Keeps a `send` caller's refusal as `refusal`, the way `post` does. */
	refuse(intent: SessionIntent, error: unknown): void;
	/** Why the latest posted intent was refused, worded for the user. */
	readonly refusal: Accessor<string | null>;
	dismissRefusal(): void;
	/** `pending` until the engine's first snapshot arrives; nothing it owns
	 * is truthful before then. */
	readonly attachment: Accessor<Attachment>;
	sendSettings(intent: SettingsIntent): Promise<SettingsOutcome>;
	ready(): Promise<void>;
	dispose(): void;
};

export type Attachment =
	| { readonly kind: 'pending' }
	| { readonly kind: 'ready' }
	| { readonly kind: 'failed'; readonly message: string };

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
		coversRevision: 0,
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
		submissionInProgress: false,
		collisionReview: null,
		restartOffers: [],
		restartPrompt: null,
		previewRun: null,
	};
}

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
		selection: null,
		refusal: null,
	};
}

function emptyRemote(): RemoteUiSnapshot {
	return {
		revision: UNATTACHED,
		lane: 'audible',
		providers: [],
		account: null,
		accountStatus: { kind: 'idle' },
		auth: { kind: 'idle' },
		libraryStatus: { kind: 'idle' },
		busy: false,
		selectedTitleIds: [],
		includePdfByTitleId: {},
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
			emptyCategoriesRefused: false,
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
	};
}

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

function shareUnchangedResults(previous: SessionLookup, next: SessionLookup): SessionLookup {
	const known = new Map(previous.results.map((result) => [JSON.stringify(result), result]));
	return {
		...next,
		results: next.results.map((result) => known.get(JSON.stringify(result)) ?? result),
	};
}

const sameText = (left: readonly string[], right: readonly string[]): boolean =>
	left.length === right.length && left.every((value, index) => value === right[index]);

function sameRecord<T>(left: Record<string, T>, right: Record<string, T>): boolean {
	const keys = Object.keys(right);
	return (
		keys.length === Object.keys(left).length &&
		keys.every((key) => key in left && JSON.stringify(left[key]) === JSON.stringify(right[key]))
	);
}

function shareUnchangedRemote(
	previous: RemoteUiSnapshot,
	next: RemoteUiSnapshot,
): RemoteUiSnapshot {
	const known = new Map(
		previous.indexer.releases.map((release) => [
			`${release.indexerId}\u0000${release.guid}`,
			{ release, content: JSON.stringify(release) },
		]),
	);
	const releases = next.indexer.releases.map((release) => {
		const existing = known.get(`${release.indexerId}\u0000${release.guid}`);
		return existing && existing.content === JSON.stringify(release) ? existing.release : release;
	});
	const previousReleases = previous.indexer.releases;
	return {
		...next,
		selectedTitleIds: sameText(previous.selectedTitleIds, next.selectedTitleIds)
			? previous.selectedTitleIds
			: next.selectedTitleIds,
		includePdfByTitleId: sameRecord(previous.includePdfByTitleId, next.includePdfByTitleId)
			? previous.includePdfByTitleId
			: next.includePdfByTitleId,
		indexer: {
			...next.indexer,
			releases:
				releases.length === previousReleases.length &&
				releases.every((release, index) => release === previousReleases[index])
					? previousReleases
					: releases,
			selectedReleaseKeys: sameText(
				previous.indexer.selectedReleaseKeys,
				next.indexer.selectedReleaseKeys,
			)
				? previous.indexer.selectedReleaseKeys
				: next.indexer.selectedReleaseKeys,
			releaseGrabs: sameRecord(previous.indexer.releaseGrabs, next.indexer.releaseGrabs)
				? previous.indexer.releaseGrabs
				: next.indexer.releaseGrabs,
		},
	};
}

type Part<T> = { readonly read: () => T; apply(next: T | null | undefined): void };

function sessionPart<T extends { readonly revision: number }>(
	initial: T,
	share: (current: T, next: T) => T = (_current, next) => next,
): Part<T> {
	let value = initial;
	const [revision, bump] = createSignal(0, { ownedWrite: true });
	return {
		read: () => {
			revision();
			return value;
		},
		apply(next) {
			if (!next || next.revision <= value.revision) return;
			value = share(value, next);
			bump((n) => n + 1);
		},
	};
}

export function createEngineLink(capability: EngineCapability = liveEngineCapability): EngineLink {
	const titles = sessionPart(emptyTitles(), shareUnchangedFiles);
	const selection = sessionPart<SessionSelection>({
		revision: UNATTACHED,
		selectedIndices: [],
		selectedAnchor: null,
	});
	const metadata = sessionPart(emptyMetadata());
	const lookup = sessionPart(emptyLookup(), shareUnchangedResults);
	const audio = sessionPart(emptyAudio());
	const output = sessionPart(emptyOutput());
	const remote = sessionPart(emptyRemote(), shareUnchangedRemote);
	const remoteLibrary = sessionPart<RemoteLibrarySnapshot>({
		revision: UNATTACHED,
		titles: [],
		diagnostics: [],
	});
	const remoteAcquisition = sessionPart<RemoteAcquisitionSnapshot>({
		revision: UNATTACHED,
		acquiring: false,
		job: null,
	});
	let settings = emptySettings();
	const part = () => createSignal(0, { ownedWrite: true });
	const [settingsRev, bumpSettings] = part();
	const [refusalRev, bumpRefusal] = part();
	const [attachmentRev, bumpAttachment] = part();
	let refusal: string | null = null;
	let attachment: Attachment = { kind: 'pending' };
	let disposed = false;
	let sessionSequence = 0;
	let settingsSequence = 0;
	let unlisten: (() => void) | undefined;

	function applySession(update: SessionUpdate): void {
		if (disposed) return;
		titles.apply(update.titles);
		selection.apply(update.selection);
		metadata.apply(update.metadata);
		remote.apply(update.remote);
		remoteAcquisition.apply(update.remoteAcquisition);
		remoteLibrary.apply(update.remoteLibrary);
		output.apply(update.output);
		audio.apply(update.audio);
		lookup.apply(update.lookup);
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
		.then((attached) => {
			applySession(attached.session);
			applySettings(attached.settings);
			setAttachment({ kind: 'ready' });
			return attached.client;
		});
	attached.catch((error: unknown) => {
		setAttachment({
			kind: 'failed',
			message: toUserMessage(error, { fallback: 'ABB could not reach its engine.' }),
		});
	});

	function setAttachment(next: Attachment): void {
		if (disposed) return;
		attachment = next;
		bumpAttachment((n) => n + 1);
	}

	function dispatch(
		request: (client: number, sequence: number) => Promise<SessionReply>,
	): Promise<SessionOutcome> {
		return attached
			.then((client) => request(client, sessionSequence++))
			.then((reply) => {
				applySession(reply.update);
				if (reply.outcome.kind === 'rejected') throw reply.outcome.error;
				return reply.outcome;
			});
	}

	function send(intent: SessionIntent): Promise<SessionOutcome> {
		return dispatch((client, sequence) => capability.sessionDispatch(client, sequence, intent));
	}

	function refuse(intent: SessionIntent, error: unknown): void {
		if (disposed || isCancellation(error)) return;
		console.error(`Session intent ${intent.kind} failed:`, error);
		refusal = toUserMessage(error, { fallback: "That change couldn't be applied." });
		bumpRefusal((n) => n + 1);
	}

	return {
		titles: titles.read,
		selection: selection.read,
		metadata: metadata.read,
		audio: audio.read,
		remote: remote.read,
		remoteLibrary: remoteLibrary.read,
		remoteAcquisition: remoteAcquisition.read,
		output: output.read,
		lookup: lookup.read,
		settings: () => {
			settingsRev();
			return settings;
		},
		send,
		openPreview: (runId) =>
			dispatch((client, sequence) => capability.openPreviewOutput(client, sequence, runId)),
		post(intent) {
			send(intent).catch((error: unknown) => refuse(intent, error));
		},
		refuse,
		refusal: () => {
			refusalRev();
			return refusal;
		},
		dismissRefusal() {
			refusal = null;
			bumpRefusal((n) => n + 1);
		},
		attachment: () => {
			attachmentRev();
			return attachment;
		},
		sendSettings(intent) {
			return attached
				.then((client) => capability.settingsDispatch(client, settingsSequence++, intent))
				.then((reply) => {
					applySettings(reply.snapshot);
					return reply.outcome;
				});
		},
		ready: () => attached.then(() => undefined),
		dispose() {
			disposed = true;
			unlisten?.();
			unlisten = undefined;
		},
	};
}
