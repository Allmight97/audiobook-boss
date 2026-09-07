import { cleanup, render, waitFor } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createTestAppRuntime } from '../../../app/runtime/harness';
import { AppRuntimeProvider } from '../../../app/runtime/RuntimeProvider';
import { liveInputCapability } from '../../../lib/tauri/capabilities/input';
import { FileListView } from '../FileListView';

function deferred() {
	let resolve!: (bytes: number[]) => void;
	const promise = new Promise<number[]>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

async function mountedList(request: ReturnType<typeof deferred>, paths = ['/shared.m4b']) {
	const load = vi.fn(() => request.promise);
	const runtime = createTestAppRuntime({
		input: {
			...liveInputCapability,
			discoverAudioImportPaths: async (paths) => [...paths],
			analyzeAudioFiles: async () => ({
				files: paths.map((path) => ({
					path,
					inputId: path,
					isValid: true,
					duration: 1,
					size: 1,
					format: 'm4b',
				})),
				selectedDecoders: paths.map(() => null),
				totalDuration: paths.length,
				totalSize: paths.length,
				validCount: paths.length,
				invalidCount: 0,
			}),
			readAudioCoverThumbnail: load,
		},
	});
	await runtime.input.importIntent({ type: 'importPaths', paths });
	const view = render(() => (
		<AppRuntimeProvider runtime={runtime}>
			<FileListView onHeaderClick={() => undefined} />
		</AppRuntimeProvider>
	));
	return { runtime, view, load };
}

describe('FileList thumbnail lifetime', () => {
	afterEach(cleanup);

	it.each(['clear', 'unmount'] as const)(
		'keeps another mounted view independent after %s',
		async (action) => {
			const firstRequest = deferred();
			const secondRequest = deferred();
			const first = await mountedList(firstRequest, ['/shared.m4b', '/second.m4b', '/queued.m4b']);
			const second = await mountedList(secondRequest);
			try {
				await waitFor(() => expect(first.load).toHaveBeenCalledTimes(2));
				await waitFor(() => expect(second.load).toHaveBeenCalledOnce());
				if (action === 'clear') await first.runtime.input.clearAllFiles();
				else first.view.unmount();
				firstRequest.resolve([1]);
				for (let index = 0; index < 12; index += 1) await Promise.resolve();
				expect(first.load).toHaveBeenCalledTimes(2);
				expect(second.view.container.querySelector('img')).toBeNull();
				secondRequest.resolve([2]);
				await waitFor(() =>
					expect(second.view.container.querySelector('img')).toHaveAttribute(
						'src',
						'data:image/jpeg;base64,Ag==',
					),
				);
			} finally {
				first.view.unmount();
				second.view.unmount();
				first.runtime.dispose();
				second.runtime.dispose();
			}
		},
	);
});
