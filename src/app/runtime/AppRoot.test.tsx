import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { useAppRuntime } from './context';
import { AppRoot } from './RuntimeProvider';

function Attached() {
	const runtime = useAppRuntime();
	return <p data-testid="attached">{runtime.input.view().fileCount} titles</p>;
}

function holdAttach(engine: FakeEngine): () => void {
	const attach = engine.attach.bind(engine);
	let release!: () => void;
	const released = new Promise<void>((resolve) => {
		release = resolve;
	});
	engine.attach = async () => {
		await released;
		return attach();
	};
	return release;
}

function renderRoot(engine: FakeEngine) {
	return render(() => (
		<AppRoot capabilities={{ engine }}>
			<Attached />
		</AppRoot>
	));
}

describe('app root', () => {
	afterEach(cleanup);

	it('shows nothing until the first snapshot arrives, then composes the app from it', async () => {
		const engine = createFakeEngine();
		const release = holdAttach(engine);
		renderRoot(engine);

		await new Promise((resolve) => setTimeout(resolve));
		expect(screen.queryByTestId('attached')).toBeNull();
		expect(screen.queryByRole('alert')).toBeNull();

		release();
		expect(await screen.findByTestId('attached')).toHaveTextContent('0 titles');
	});

	it('says so when the engine cannot be reached', async () => {
		const engine = createFakeEngine();
		engine.attach = async () => {
			throw new Error('engine unavailable');
		};
		renderRoot(engine);

		expect(await screen.findByRole('alert')).toHaveTextContent('engine unavailable');
		expect(screen.queryByTestId('attached')).toBeNull();
	});

	it('words an engine failure that carries no message of its own', async () => {
		const engine = createFakeEngine();
		engine.attach = () => Promise.reject(undefined);
		renderRoot(engine);

		expect(await screen.findByRole('alert')).toHaveTextContent('ABB could not reach its engine.');
	});

	it('leaves nothing listening when it unmounts before the engine attaches', async () => {
		const engine = createFakeEngine();
		const stopped = vi.fn();
		const listen = engine.listenSessionUpdates.bind(engine);
		engine.listenSessionUpdates = async (handler) => {
			const stop = await listen(handler);
			return () => {
				stopped();
				stop();
			};
		};
		const release = holdAttach(engine);
		const { unmount } = renderRoot(engine);

		unmount();
		release();

		await vi.waitFor(() => expect(stopped).toHaveBeenCalledOnce());
	});

	it('disposes the app it composed when it unmounts', async () => {
		const engine = createFakeEngine();
		const stopped = vi.fn();
		const listen = engine.listenSessionUpdates.bind(engine);
		engine.listenSessionUpdates = async (handler) => {
			const stop = await listen(handler);
			return () => {
				stopped();
				stop();
			};
		};
		const { unmount } = renderRoot(engine);
		await screen.findByTestId('attached');

		unmount();

		expect(stopped).toHaveBeenCalledOnce();
	});
});
