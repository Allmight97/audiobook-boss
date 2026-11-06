import { createRoot } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import type { SessionUpdate } from '../../types/session';
import type { OperationSnapshot } from '../../types/workRuntime';
import { createEngineLink, type EngineLink } from './link';

function operation(operationId: string): OperationSnapshot {
	return {
		operationId,
		sequence: 1,
		revision: 1,
		kind: 'processingBatch',
		status: 'running',
		title: 'Listed',
		cancellable: true,
		cancelRequested: false,
		sourceInputIds: [],
		progress: { stage: 'converting', percentage: 10, message: 'Working.' },
		children: [],
		logTail: [],
	};
}

describe('engine link', () => {
	let dispose: (() => void) | undefined;

	afterEach(() => {
		dispose?.();
		dispose = undefined;
	});

	function linkTo(engine: FakeEngine): EngineLink {
		return createRoot((disposeRoot) => {
			const link = createEngineLink(engine);
			dispose = () => {
				link.dispose();
				disposeRoot();
			};
			return link;
		});
	}

	it('numbers intents in the order they were sent, including before the engine answers', async () => {
		const engine = createFakeEngine();
		const sequences: number[] = [];
		const dispatch = engine.sessionDispatch.bind(engine);
		engine.sessionDispatch = (client, sequence, intent) => {
			sequences.push(sequence);
			return dispatch(client, sequence, intent);
		};
		const link = linkTo(engine);

		const sent = [
			link.send({ kind: 'setField', binding: 0, field: 'title', value: 'A' }),
			link.send({ kind: 'save' }),
			link.send({ kind: 'selectAll' }),
		];
		await Promise.all(sent);

		expect(sequences).toEqual([0, 1, 2]);
		expect(engine.sessionIntents.map((intent) => intent.kind)).toEqual([
			'setField',
			'save',
			'selectAll',
		]);
	});

	it('keeps the newest copy of each part when a reply and an event cross', async () => {
		const engine = createFakeEngine();
		let publish!: (update: SessionUpdate) => void;
		engine.listenSessionUpdates = async (handler) => {
			publish = handler;
			return () => undefined;
		};
		const link = linkTo(engine);
		await link.ready();
		await link.send({ kind: 'setField', binding: 0, field: 'title', value: 'Newer' });
		const current = link.metadata();

		publish({
			revision: current.revision - 1,
			metadata: { ...current, revision: current.revision - 1, saveInProgress: true },
		});

		expect(link.metadata()).toBe(current);

		publish({
			revision: current.revision + 5,
			metadata: { ...current, revision: current.revision + 5, saveInProgress: true },
		});
		expect(link.metadata().saveInProgress).toBe(true);
	});

	it('preserves library rows on progress-only events and rejects an older library part independently', async () => {
		const engine = createFakeEngine();
		let publish!: (update: SessionUpdate) => void;
		engine.listenSessionUpdates = async (handler) => {
			publish = handler;
			return () => undefined;
		};
		const link = linkTo(engine);
		await link.ready();
		const library = { ...link.remoteLibrary(), revision: 10, diagnostics: [] };
		publish({ revision: 10, remoteLibrary: library });
		publish({
			revision: 11,
			remoteAcquisition: { revision: 11, acquiring: true, job: null },
		});
		expect(link.remoteLibrary()).toBe(library);
		publish({ revision: 12, remoteLibrary: { ...library, revision: 9, diagnostics: [] } });
		expect(link.remoteLibrary()).toBe(library);
		expect(link.remoteAcquisition().acquiring).toBe(true);
	});

	it('keeps the object of every file whose content did not change', async () => {
		const engine = createFakeEngine();
		engine.loadTitles([audioFile('/books/a.m4b'), audioFile('/books/b.m4b')]);
		const link = linkTo(engine);
		await link.ready();
		const [first, second] = link.titles().files;

		engine.change((state) => {
			state.titles.files = [{ ...state.titles.files[0], label: 'Renamed' }, state.titles.files[1]];
		});

		expect(link.titles().files[0]).not.toBe(first);
		expect(link.titles().files[0]?.label).toBe('Renamed');
		expect(link.titles().files[1]).toBe(second);
	});

	it('keeps a selection when a newer download tick arrives before its remote part', async () => {
		const engine = createFakeEngine();
		let publish!: (update: SessionUpdate) => void;
		engine.listenSessionUpdates = async (handler) => {
			publish = handler;
			return () => undefined;
		};
		const link = linkTo(engine);
		await link.ready();
		const remote = link.remote();
		publish({ revision: 31, remoteAcquisition: { revision: 31, acquiring: false, job: null } });
		publish({ revision: 30, remote: { ...remote, revision: 30, selectedTitleIds: ['picked'] } });
		expect(link.remote().selectedTitleIds).toEqual(['picked']);
		expect(link.remoteAcquisition().revision).toBe(31);
		publish({ revision: 32, remoteAcquisition: { revision: 29, acquiring: true, job: null } });
		expect(link.remoteAcquisition().acquiring).toBe(false);
	});

	it('keeps unchanged remote choices and releases when another remote fact changes', async () => {
		const release = (guid: string, seeders: number) => ({
			providerId: 'indexer' as const,
			key: guid,
			guid,
			indexerId: 1,
			title: guid,
			indexer: 'Indexer',
			sizeBytes: 1,
			protocol: 'torrent' as const,
			seeders,
			categories: [],
		});
		const engine = createFakeEngine();
		engine.change((state) => {
			state.remote.selectedTitleIds = ['a'];
			state.remote.includePdfByTitleId = { a: true, b: false };
			state.remote.indexer.releases = [release('one', 1), release('two', 2)];
			state.remote.indexer.releaseGrabs = { x: { status: 'sent', message: '' } };
		});
		const link = linkTo(engine);
		await link.ready();
		const before = link.remote();

		engine.change((state) => {
			state.remote.indexer.message = 'Searching';
			state.remote.indexer.releases = [release('one', 1), release('two', 9)];
		});

		const after = link.remote();
		expect(after.indexer.message).toBe('Searching');
		expect(after.selectedTitleIds).toBe(before.selectedTitleIds);
		expect(after.includePdfByTitleId).toBe(before.includePdfByTitleId);
		expect(after.indexer.releaseGrabs).toBe(before.indexer.releaseGrabs);
		expect(after.indexer.releases[0]).toBe(before.indexer.releases[0]);
		expect(after.indexer.releases[1]).not.toBe(before.indexer.releases[1]);
		expect(after.indexer.releases[1]?.seeders).toBe(9);
	});

	it('does not replace a live frontend when a disposed link finishes registering listeners', async () => {
		const engine = createFakeEngine();
		let registered!: (stop: () => void) => void;
		let stopped = false;
		let attachments = 0;
		engine.listenSessionUpdates = () =>
			new Promise((resolve) => {
				registered = resolve;
			});
		const attach = engine.attach.bind(engine);
		engine.attach = () => {
			attachments++;
			return attach();
		};
		const link = linkTo(engine);
		link.dispose();
		registered(() => {
			stopped = true;
		});
		await expect(link.ready()).rejects.toThrow('disposed');
		expect(attachments).toBe(0);
		expect(stopped).toBe(true);
	});

	it("shows the attach reply's operations and keeps an update that arrived before it", async () => {
		const engine = createFakeEngine();
		const listed = operation('listed');
		engine.publishOperation(listed);
		const attach = engine.attach.bind(engine);
		let reply: (() => void) | undefined;
		engine.attach = async () => {
			const attachment = await attach();
			await new Promise<void>((resolve) => {
				reply = resolve;
			});
			return attachment;
		};
		const link = linkTo(engine);
		await vi.waitFor(() => expect(reply).toBeDefined());

		engine.publishOperation({ ...listed, revision: 2, title: 'From the event' });
		reply?.();
		await link.ready();

		expect(link.workOperations().map((item) => item.title)).toEqual(['From the event']);
	});

	it('stops following operations once disposed', async () => {
		const engine = createFakeEngine();
		const link = linkTo(engine);
		await link.ready();
		link.dispose();

		engine.publishOperation(operation('late'));

		expect(link.workOperations()).toEqual([]);
	});

	it('reports a refused attach through the intent that needed it', async () => {
		const engine = createFakeEngine();
		engine.attach = async () => {
			throw new Error('engine unavailable');
		};
		const link = linkTo(engine);

		await expect(link.send({ kind: 'selectAll' })).rejects.toThrow('engine unavailable');
		await expect(link.ready()).rejects.toThrow('engine unavailable');
		expect(link.attachment()).toEqual({ kind: 'failed', message: 'engine unavailable' });
	});

	it('is pending until the first snapshot arrives', async () => {
		const link = linkTo(createFakeEngine());
		expect(link.attachment()).toEqual({ kind: 'pending' });
		await link.ready();
		expect(link.attachment()).toEqual({ kind: 'ready' });
	});
});
