import { createSignal, type Accessor } from 'solid-js';
import { logFrontendFailure } from '../../lib/frontendLogBridge';
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
import type { OperationId, OperationSnapshot, WorkOperationsUpdate } from '../../types/workRuntime';
import { applyWorkOperations, emptyWorkCenterModel, visibleOperations } from './workOperations';

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
	/** WorkRuntime's operations in the engine's display order. */
	readonly workOperations: Accessor<ReadonlyArray<OperationSnapshot>>;
	/** Cancels an operation, or only the title named by `childJobId`; the
	 * change arrives in `workOperations`. */
	cancelWork: (operationId: OperationId, childJobId?: string) => Promise<void>;
	/** Sends an intent and resolves with its outcome once its work has finished. */
	send: (intent: SessionIntent) => Promise<SessionOutcome>;
	openPreview: (runId: string) => Promise<SessionOutcome>;
	/** Sends an intent whose outcome the caller does not need. A refusal is
	 * kept as `refusal` until dismissed. */
	post: (intent: SessionIntent) => void;
	/** Keeps a `send` caller's refusal as `refusal`, the way `post` does.
	 * `scope` names what failed in the log: an intent kind, or the step. */
	refuse: (scope: string, error: unknown) => void;
	/** Why the latest posted intent was refused, worded for the user. */
	readonly refusal: Accessor<string | null>;
	dismissRefusal: () => void;
	sendSettings: (intent: SettingsIntent) => Promise<SettingsOutcome>;
	dispose: () => void;
};

const SAVE_RUNNING_NOTICE = 'A Save is running. Try again when it finishes.';
const EDIT_LOST_NOTICE = 'Your edit was not applied because the selection changed.';

const SUPERSEDED_LOSES_THE_EDIT: ReadonlySet<SessionIntent['kind']> = new Set([
	'setField',
	'setFieldAction',
]);

const COVER_LOADS: ReadonlySet<SessionIntent['kind']> = new Set([
	'loadCoverFromFile',
	'loadCoverFromDrop',
	'loadCoverFromUrl',
]);

function outcomeNotice(
	intent: SessionIntent,
	outcome: SessionOutcome,
	formChanged: boolean,
): string | null {
	if (outcome.kind === 'draftRejected' && outcome.message === null) return SAVE_RUNNING_NOTICE;
	if (outcome.kind !== 'superseded') return null;
	if (SUPERSEDED_LOSES_THE_EDIT.has(intent.kind)) return EDIT_LOST_NOTICE;
	return COVER_LOADS.has(intent.kind) && formChanged ? EDIT_LOST_NOTICE : null;
}

function shareUnchangedFiles(previous: SessionTitles, next: SessionTitles): SessionTitles {
	const known = new Map<string, { file: AudioFile; content: string }>();
	const remember = (file: AudioFile) =>
		known.set(file.inputId ?? file.path, {
			file,
			content: JSON.stringify(file),
		});
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
			release.key,
			{ release, content: JSON.stringify(release) },
		]),
	);
	const releases = next.indexer.releases.map((release) => {
		const existing = known.get(release.key);
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

type Attachment = Awaited<ReturnType<EngineCapability['attach']>>;

type Inbound = {
	session: (update: SessionUpdate) => void;
	settings: (snapshot: SettingsSnapshot) => void;
	work: (update: WorkOperationsUpdate) => void;
};

type Part<T> = {
	readonly read: () => T;
	apply(next: T | null | undefined): void;
};

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

function present<T>(part: T | null, name: string): T {
	if (part === null) throw new Error(`The engine's first snapshot has no ${name}.`);
	return part;
}

function createLink(
	capability: EngineCapability,
	attachment: Attachment,
	stopListening: () => void,
): { readonly link: EngineLink; readonly inbound: Inbound } {
	const { client, session } = attachment;
	const titles = sessionPart(present(session.titles, 'titles'), shareUnchangedFiles);
	const selection = sessionPart(present(session.selection, 'selection'));
	const metadata = sessionPart(present(session.metadata, 'metadata'));
	const lookup = sessionPart(present(session.lookup, 'lookup'), shareUnchangedResults);
	const audio = sessionPart(present(session.audio, 'audio'));
	const output = sessionPart(present(session.output, 'output'));
	const remote = sessionPart(present(session.remote, 'remote'), shareUnchangedRemote);
	const remoteLibrary = sessionPart(present(session.remoteLibrary, 'remote library'));
	const remoteAcquisition = sessionPart(present(session.remoteAcquisition, 'remote acquisition'));
	const settings = sessionPart(attachment.settings);
	let work = applyWorkOperations(emptyWorkCenterModel(), attachment.workOperations);
	let operations: ReadonlyArray<OperationSnapshot> = visibleOperations(work);
	const part = () => createSignal(0, { ownedWrite: true });
	const [workRev, bumpWork] = part();
	const [refusalRev, bumpRefusal] = part();
	let refusal: string | null = null;
	let disposed = false;
	let sessionSequence = 0;
	let settingsSequence = 0;

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
		if (!disposed) settings.apply(snapshot);
	}

	function applyWork(incoming: WorkOperationsUpdate): void {
		if (disposed) return;
		const next = applyWorkOperations(work, incoming);
		if (next === work) return;
		work = next;
		operations = visibleOperations(work);
		bumpWork((n) => n + 1);
	}

	function dispatch(
		request: (client: number, sequence: number) => Promise<SessionReply>,
	): Promise<SessionOutcome> {
		return request(client, sessionSequence++).then((reply) => {
			applySession(reply.update);
			if (reply.outcome.kind === 'rejected') throw reply.outcome.error;
			return reply.outcome;
		});
	}

	function showRefusal(message: string): void {
		if (disposed) return;
		refusal = message;
		bumpRefusal((n) => n + 1);
	}

	function send(intent: SessionIntent): Promise<SessionOutcome> {
		const bindingAtSend = metadata.read().binding;
		return dispatch((id, sequence) => capability.sessionDispatch(id, sequence, intent)).then(
			(outcome) => {
				const notice = outcomeNotice(intent, outcome, metadata.read().binding !== bindingAtSend);
				if (notice !== null) showRefusal(notice);
				return outcome;
			},
		);
	}

	function refuse(scope: string, error: unknown): void {
		if (disposed || isCancellation(error)) return;
		logFrontendFailure(`engineLink.${scope}`, error);
		showRefusal(toUserMessage(error, { fallback: "That change couldn't be applied." }));
	}

	const link: EngineLink = {
		titles: titles.read,
		selection: selection.read,
		metadata: metadata.read,
		audio: audio.read,
		remote: remote.read,
		remoteLibrary: remoteLibrary.read,
		remoteAcquisition: remoteAcquisition.read,
		output: output.read,
		lookup: lookup.read,
		settings: settings.read,
		workOperations: () => {
			workRev();
			return operations;
		},
		cancelWork: capability.cancelWorkOperation,
		send,
		openPreview: (runId) =>
			dispatch((id, sequence) => capability.openPreviewOutput(id, sequence, runId)),
		post(intent) {
			send(intent).catch((error: unknown) => refuse(intent.kind, error));
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
		sendSettings(intent) {
			return capability.settingsDispatch(client, settingsSequence++, intent).then((reply) => {
				applySettings(reply.snapshot);
				return reply.outcome;
			});
		},
		dispose() {
			disposed = true;
			stopListening();
		},
	};
	return {
		link,
		inbound: {
			session: applySession,
			settings: applySettings,
			work: applyWork,
		},
	};
}

/**
 * Starts listening, then attaches, and resolves with the link once the
 * engine's first snapshot is in it. What arrives in between is applied on top
 * of that snapshot, so no update is missed; part revisions keep the newest.
 */
export async function attachEngineLink(
	capability: EngineCapability = liveEngineCapability,
): Promise<EngineLink> {
	const early: Array<(inbound: Inbound) => void> = [];
	let inbound: Inbound | undefined;
	function deliver(apply: (to: Inbound) => void): void {
		if (inbound) apply(inbound);
		else early.push(apply);
	}
	const listeners = await Promise.allSettled([
		capability.listenSessionUpdates((update) => deliver((to) => to.session(update))),
		capability.listenSettingsUpdates((snapshot) => deliver((to) => to.settings(snapshot))),
		capability.listenWorkOperations((update) => deliver((to) => to.work(update))),
	]);
	const stopListening = () => {
		for (const listener of listeners) {
			if (listener.status === 'fulfilled') listener.value();
		}
	};
	try {
		for (const listener of listeners) {
			if (listener.status === 'rejected') throw listener.reason;
		}
		const attached = createLink(capability, await capability.attach(), stopListening);
		inbound = attached.inbound;
		for (const apply of early) apply(attached.inbound);
		return attached.link;
	} catch (error) {
		stopListening();
		throw error;
	}
}
