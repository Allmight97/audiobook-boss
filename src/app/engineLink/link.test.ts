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

	it('reports a refused attach through the intent that needed it', async () => {
		const engine = createFakeEngine();
		engine.attach = async () => {
			throw new Error('engine unavailable');
		};
		const link = linkTo(engine);

		await expect(link.send({ kind: 'selectAll' })).rejects.toThrow('engine unavailable');
		await expect(link.ready()).rejects.toThrow('engine unavailable');
	});
});
