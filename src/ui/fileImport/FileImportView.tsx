import { createEffect, onCleanup, onMount, Show, type JSX } from 'solid-js';
import {
	nativeDropLooksLikeCoverArt,
	nativeDropTargetAtPoint,
} from '../../app/inputSession/nativeIngress';
import { useAppRuntime } from '../../app/runtime';
import { isFileDropEvent } from '../../types/events';
import { createSubscriptionGroup } from '../../lib/tauri/subscriptionGroup';
import { Button } from '../foundation';
import { FileListView } from '../fileList/FileListView';
import { RemoteSourceAcquireView } from '../remoteSource/RemoteSourceAcquireView';
import './fileImport.css';

export function FileImportView(): JSX.Element {
	const runtime = useAppRuntime();
	const view = runtime.input.view;
	const capability = runtime.input.capability;
	const importIntent = runtime.input.importIntent;
	const openRemoteSourceAcquire = runtime.remoteSource.open;
	const applyCoverArtDrop = runtime.metadata.applyCoverArtDrop;
	const hydrateSupportText = runtime.input.hydrateSupportText;
	const setDragOver = runtime.input.setDragOver;
	createEffect(() => {
		runtime.remoteSource.reconcileWithInput(view().files);
	});
	let fileManagementContainer: HTMLElement | null = null;
	let deferredOpenedDrain = false;

	async function drainOpenedAudioFiles(): Promise<void> {
		if (view().orderLocked) {
			deferredOpenedDrain = true;
			importIntent({ type: 'drainOpened' });
			return;
		}
		deferredOpenedDrain = false;
		importIntent({ type: 'drainOpened' });
	}

	onMount(() => {
		void hydrateSupportText();
		const subscriptions = createSubscriptionGroup();
		void subscriptions.add(
			capability().listenDragEnter(() => {
				setDragOver(true);
			}),
		);
		void subscriptions.add(
			capability().listenDragLeave(() => {
				setDragOver(false);
			}),
		);
		void subscriptions.add(
			capability().listenDragDrop((payload) => {
				setDragOver(false);
				if (!isFileDropEvent(payload)) {
					return;
				}
				const coverArea = document.getElementById('cover-art-area');
				const coverHit = nativeDropTargetAtPoint(payload.position, coverArea, null) === 'cover';
				const filesHit =
					nativeDropTargetAtPoint(payload.position, null, fileManagementContainer) === 'files';
				if (coverHit && nativeDropLooksLikeCoverArt(payload.paths)) {
					void applyCoverArtDrop([...payload.paths]);
					return;
				}
				if (filesHit) {
					importIntent({ type: 'importPaths', paths: [...payload.paths] });
				}
			}),
		);
		void subscriptions.add(
			capability().listenOpenedAudioFiles(() => {
				void drainOpenedAudioFiles();
			}),
		);
		void drainOpenedAudioFiles();
		onCleanup(() => subscriptions.dispose());
	});

	createEffect(() => {
		if (!view().orderLocked && deferredOpenedDrain) {
			void drainOpenedAudioFiles();
		}
	});

	return (
		<>
			<div class="file-import-actions">
				<Button id="add-folder-btn" onClick={() => importIntent({ type: 'pickFolder' })}>
					Add Folder
				</Button>
				<Button id="acquire-audiobooks-btn" onClick={() => openRemoteSourceAcquire()}>
					Import from Library
				</Button>
			</div>
			<Show when={view().errorMessage}>
				{(message) => (
					<div id="file-import-error" class="file-import-error">
						{message()}
					</div>
				)}
			</Show>
			<FileListView
				onHeaderClick={() => importIntent({ type: 'pickFiles' })}
				fileManagementRef={(element) => {
					fileManagementContainer = element;
				}}
			/>
			<RemoteSourceAcquireView />
		</>
	);
}
