import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	audioFile,
	createFakeEngine,
	sessionUpdate,
	type FakeEngine,
} from '../../test/fixtures/fakeEngine';
import type {
	OperationSnapshot,
	SessionIntent,
	SessionOutcome,
	SessionUpdate,
} from '../../lib/generated/tauri';
import { attachEngineLink, type EngineLink } from './link';

function operation(operationId: string): OperationSnapshot {
	return {
		operationId,
		sequence: 1,
		revision: 1,
		kind: 'processingBatch',
		status: 'running',
		title: 'Listed',
		startedAtMs: null,
		finishedAtMs: null,
		cancellable: true,
		cancelRequested: false,
		progress: {
			stage: 'converting',
			percentage: 10,
			message: 'Working.',
			etaSeconds: null,
		},
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

	async function linkTo(engine: FakeEngine): Promise<EngineLink> {
		const link = await attachEngineLink(engine);
		dispose = () => link.dispose();
		return link;
	}

	it('numbers intents in the order they were sent, including before the engine answers', async () => {
		const engine = createFakeEngine();
		const sequences: number[] = [];
		const dispatch = engine.sessionDispatch.bind(engine);
		engine.sessionDispatch = (client, sequence, intent) => {
			sequences.push(sequence);
			return dispatch(client, sequence, intent);
		};
		const link = await linkTo(engine);

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
		const link = await linkTo(engine);
		await link.send({
			kind: 'setField',
			binding: 0,
			field: 'title',
			value: 'Newer',
		});
		const current = link.metadata();

		publish(
			sessionUpdate({
				revision: current.revision - 1,
				metadata: {
					...current,
					revision: current.revision - 1,
					saveInProgress: true,
				},
			}),
		);

		expect(link.metadata()).toBe(current);

		publish(
			sessionUpdate({
				revision: current.revision + 5,
				metadata: {
					...current,
					revision: current.revision + 5,
					saveInProgress: true,
				},
			}),
		);
		expect(link.metadata().saveInProgress).toBe(true);
	});

	it('preserves library rows on progress-only events and rejects an older library part independently', async () => {
		const engine = createFakeEngine();
		let publish!: (update: SessionUpdate) => void;
		engine.listenSessionUpdates = async (handler) => {
			publish = handler;
			return () => undefined;
		};
		const link = await linkTo(engine);
		const library = { ...link.remoteLibrary(), revision: 10, diagnostics: [] };
		publish(sessionUpdate({ revision: 10, remoteLibrary: library }));
		publish(
			sessionUpdate({
				revision: 11,
				remoteAcquisition: { revision: 11, job: null },
			}),
		);
		expect(link.remoteLibrary()).toBe(library);
		publish(
			sessionUpdate({
				revision: 12,
				remoteLibrary: { ...library, revision: 9, diagnostics: [] },
			}),
		);
		expect(link.remoteLibrary()).toBe(library);
		expect(link.remoteAcquisition().revision).toBe(11);
	});

	it('keeps the object of every file whose content did not change', async () => {
		const engine = createFakeEngine();
		engine.loadTitles([audioFile('/books/a.m4b'), audioFile('/books/b.m4b')]);
		const link = await linkTo(engine);
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
		const link = await linkTo(engine);
		const remote = link.remote();
		publish(
			sessionUpdate({
				revision: 31,
				remoteAcquisition: { revision: 31, job: null },
			}),
		);
		publish(
			sessionUpdate({
				revision: 30,
				remote: { ...remote, revision: 30, selectedTitleIds: ['picked'] },
			}),
		);
		expect(link.remote().selectedTitleIds).toEqual(['picked']);
		expect(link.remoteAcquisition().revision).toBe(31);
		publish(
			sessionUpdate({
				revision: 32,
				remoteAcquisition: { revision: 29, job: null },
			}),
		);
		expect(link.remoteAcquisition().revision).toBe(31);
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
			detailUrl: null,
			seeders,
			categories: [],
		});
		const engine = createFakeEngine();
		engine.change((state) => {
			state.remote.selectedTitleIds = ['a'];
			state.remote.includePdfByTitleId = { a: true, b: false };
			state.remote.indexer.releases = [release('one', 1), release('two', 2)];
			state.remote.indexer.releaseGrabs = {
				x: { status: 'sent', message: '' },
			};
		});
		const link = await linkTo(engine);
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

	function holdAttachReply(engine: FakeEngine): {
		replying: () => boolean;
		reply: () => void;
	} {
		const attach = engine.attach.bind(engine);
		let reply: (() => void) | undefined;
		engine.attach = async () => {
			const attachment = await attach();
			await new Promise<void>((resolve) => {
				reply = resolve;
			});
			return attachment;
		};
		return { replying: () => reply !== undefined, reply: () => reply?.() };
	}

	it("shows the attach reply's operations and keeps an update that arrived before it", async () => {
		const engine = createFakeEngine();
		const listed = operation('listed');
		engine.publishOperation(listed);
		const held = holdAttachReply(engine);
		const attaching = linkTo(engine);
		await vi.waitFor(() => expect(held.replying()).toBe(true));

		engine.publishOperation({
			...listed,
			revision: 2,
			title: 'From the event',
		});
		held.reply();
		const link = await attaching;

		expect(link.workOperations().map((item) => item.title)).toEqual(['From the event']);
	});

	it('keeps a session update and a settings update that arrived before the attach reply', async () => {
		const engine = createFakeEngine();
		const held = holdAttachReply(engine);
		const attaching = linkTo(engine);
		await vi.waitFor(() => expect(held.replying()).toBe(true));

		engine.change((state) => {
			state.output.directory = '/from-the-event';
		});
		engine.seedAppSettings({ keepAwakeWhileWorking: false });
		held.reply();
		const link = await attaching;

		expect(link.output().directory).toBe('/from-the-event');
		expect(link.settings().settings.keepAwakeWhileWorking).toBe(false);
	});

	it('stops following operations once disposed', async () => {
		const engine = createFakeEngine();
		const link = await linkTo(engine);
		link.dispose();

		engine.publishOperation(operation('late'));

		expect(link.workOperations()).toEqual([]);
	});

	describe('outcomes that change nothing and say nothing', () => {
		const saveRunning = 'A Save is running. Try again when it finishes.';
		const editLost = 'Your edit was not applied because the selection changed.';

		async function refusalAfter(intent: SessionIntent, outcome: SessionOutcome) {
			const engine = createFakeEngine();
			engine.respond = () => outcome;
			const link = await linkTo(engine);
			expect(await link.send(intent)).toEqual(outcome);
			return link.refusal();
		}

		it.each<[SessionIntent]>([
			[{ kind: 'selectAll' }],
			[{ kind: 'clearSelection' }],
			[
				{
					kind: 'selectFile',
					titleId: 'a',
					modifiers: { multi: false, range: false },
				},
			],
		])('says a Save is running when it blocks %j', async (intent) => {
			expect(await refusalAfter(intent, { kind: 'draftRejected', message: null })).toBe(
				saveRunning,
			);
		});

		it('leaves a rejected draft to the message the form already shows', async () => {
			expect(
				await refusalAfter({ kind: 'selectAll' }, { kind: 'draftRejected', message: 'Bad date' }),
			).toBeNull();
		});

		it.each<[SessionIntent]>([
			[{ kind: 'setField', binding: 0, field: 'title', value: 'A' }],
			[{ kind: 'setFieldAction', binding: 0, field: 'title', action: 'blank' }],
			[{ kind: 'loadCoverFromFile', path: '/c.jpg' }],
			[{ kind: 'loadCoverFromUrl', url: 'https://example.com/c.jpg' }],
		])('says the edit was lost when the engine reports %j as lost', async (intent) => {
			expect(await refusalAfter(intent, { kind: 'editLost' })).toBe(editLost);
		});

		it.each<[SessionIntent]>([
			[
				{
					kind: 'selectFile',
					titleId: 'a',
					modifiers: { multi: false, range: false },
				},
			],
			[{ kind: 'chooseCollisionPolicy', reviewId: 1, policy: 'skip_existing' }],
			[{ kind: 'loadCoverFromUrl', url: 'https://example.com/c.jpg' }],
		])('stays silent when a newer %j replaces it', async (intent) => {
			expect(await refusalAfter(intent, { kind: 'superseded' })).toBeNull();
		});
	});

	it('stops listening and rejects when the engine refuses to attach', async () => {
		const engine = createFakeEngine();
		const listen = engine.listenSessionUpdates.bind(engine);
		let stopped = false;
		engine.listenSessionUpdates = async (handler) => {
			const stop = await listen(handler);
			return () => {
				stopped = true;
				stop();
			};
		};
		engine.attach = async () => {
			throw new Error('engine unavailable');
		};

		await expect(attachEngineLink(engine)).rejects.toThrow('engine unavailable');
		expect(stopped).toBe(true);
	});

	it('stops the listeners that started when another one cannot', async () => {
		const engine = createFakeEngine();
		const listen = engine.listenSessionUpdates.bind(engine);
		let stopped = false;
		engine.listenSessionUpdates = async (handler) => {
			const stop = await listen(handler);
			return () => {
				stopped = true;
				stop();
			};
		};
		engine.listenWorkOperations = async () => {
			throw new Error('listener unavailable');
		};

		await expect(attachEngineLink(engine)).rejects.toThrow('listener unavailable');
		expect(stopped).toBe(true);
	});

	it('refuses a first snapshot that lacks a part', async () => {
		const engine = createFakeEngine();
		const attach = engine.attach.bind(engine);
		engine.attach = async () => {
			const attachment = await attach();
			return {
				...attachment,
				session: { ...attachment.session, output: null },
			};
		};

		await expect(attachEngineLink(engine)).rejects.toThrow('output');
	});
});
