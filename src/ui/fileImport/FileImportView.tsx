import { createEffect, onCleanup, onMount, Show, type JSX } from 'solid-js';

import {
	nativeDropLooksLikeCoverArt,
	nativeDropTargetAtPoint,
} from '../../app/inputSession/nativeIngress';
import { useAppRuntime } from '../../app/runtime';
import type { AcquisitionLane } from '../../types/appSettings';
import { isFileDropEvent } from '../../types/events';
import { createSubscriptionGroup } from '../../lib/tauri/subscriptionGroup';
import { Button, SplitButton } from '../foundation';
import { FileListView } from '../fileList/FileListView';
import { RemoteSourceAcquireView } from '../remoteSource';
import './fileImport.css';

export function FileImportView(): JSX.Element {
	const runtime = useAppRuntime();
	const view = runtime.input.view;
	const capability = runtime.input.capability;
	const importIntent = runtime.input.importIntent;
	const remoteSource = runtime.remoteSource;
	const defaultAcquisitionLane = runtime.settings.defaultAcquisitionLane;
	const applyCoverArtDrop = runtime.metadata.applyCoverArtDrop;
	const hydrateSupportText = runtime.input.hydrateSupportText;
	const setDragOver = runtime.input.setDragOver;
	createEffect(() => {
		void runtime.remoteSource.reconcileWithInput(view().files);
	});
	let fileManagementContainer: HTMLElement | null = null;
	let deferredOpenedDrain = false;

	function openAcquire(lane: AcquisitionLane): void {
		remoteSource.open({ lane });
	}

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
				<SplitButton
					testId="import-split-button"
					mainId="acquire-audiobooks-btn"
					caretId="import-split-caret"
					dropdownId="import-split-dropdown"
					mainLabel="Import"
					onMainClick={() => openAcquire(defaultAcquisitionLane())}
				>
					{({ close }) => (
						<>
							<SplitButton.Option
								data-testid="import-lane-audible"
								onClick={() => {
									close();
									openAcquire('audible');
								}}
							>
								Audible
							</SplitButton.Option>
							<SplitButton.Option
								data-testid="import-lane-indexer"
								onClick={() => {
									close();
									openAcquire('indexer');
								}}
							>
								Indexer
							</SplitButton.Option>
						</>
					)}
				</SplitButton>
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
