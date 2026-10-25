import { createRoot } from 'solid-js';
import { afterEach, describe, expect, it } from 'vitest';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import type { SessionUpdate } from '../../types/session';
import { createEngineLink, type EngineLink } from './link';

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

		// Sent before attach has finished; the second does not wait for the first.
		const sent = [
			link.send({ kind: 'setField', field: 'title', value: 'A' }),
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
		await link.send({ kind: 'setField', field: 'title', value: 'Newer' });
		const current = link.metadata();

		// An event carrying an older copy of the part arrives late.
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
			state.titles.files = [
				{ ...state.titles.files[0], tagTitle: 'Renamed' },
				state.titles.files[1],
			];
		});

		expect(link.titles().files[0]).not.toBe(first);
		expect(link.titles().files[0]?.tagTitle).toBe('Renamed');
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
