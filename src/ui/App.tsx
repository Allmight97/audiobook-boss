import { onSettled, Show } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { useAppRuntime } from '../app/runtime';
import { AppSettingsDialogView, SettingsPersistenceNotice } from './appSettings';
import { CollisionDialogView } from './collisionDialog';
import { SelectedAudioSettings } from './fileList';
import { OutputView } from './outputPanel';
import { FileImportView } from './fileImport';
import { ConcurrencyControl, GroupTitlesButton } from './jobControls';
import { FileInspectorView } from './leftColumn';
import { MetadataLookupView } from './metadataLookup';
import { MetadataManagerView } from './metadataManager';
import { PreviewAudioControls } from './previewAudio';
import { StatusPanelView } from './statusPanel';
import { TagPreviewView } from './tagPreview';
import { WorkCenterView } from './workCenter';
import { RefusedChangeNotice } from './RefusedChangeNotice';
import './outputAndTags.css';

export function App(): JSX.Element {
	const runtime = useAppRuntime();
	const saveMetadata = runtime.metadata.save;
	const openSettings = runtime.settings.openDialog;
	const initializeWork = runtime.workOperations.initialize;

	onSettled(() => {
		void runtime.initialize().catch((error: unknown) => {
			console.warn('Could not load startup defaults:', error);
		});
		void initializeWork();

		function handleGlobalKeyDown(event: KeyboardEvent): void {
			if ((event.metaKey || event.ctrlKey) && event.key === 's') {
				event.preventDefault();
				void saveMetadata();
			}
			if ((event.metaKey || event.ctrlKey) && event.key === ',') {
				event.preventDefault();
				void openSettings();
			}
		}
		window.addEventListener('keydown', handleGlobalKeyDown);
		return () => window.removeEventListener('keydown', handleGlobalKeyDown);
	});

	const attachment = runtime.engine.attachment;
	return (
		<Show
			when={attachment().kind === 'ready'}
			fallback={
				<Show when={attachment().kind === 'failed'}>
					<p class="engine-attach-failed" role="alert">
						{(() => {
							const current = attachment();
							return current.kind === 'failed' ? current.message : '';
						})()}
					</p>
				</Show>
			}
		>
			<div class="main-container">
				<div class="panel input-panel left-column-wrapper" data-testid="left-column">
					<section
						class="left-column-panel input-workflow input-workflow-panel"
						data-testid="input-workflow-panel"
						aria-label="Input and File Order"
					>
						<div class="input-workflow-heading-row">
							<h3 class="section-title input-workflow-heading">Input and File Order</h3>
							<div class="input-workflow-controls">
								<GroupTitlesButton />
								<SelectedAudioSettings />
								<ConcurrencyControl />
							</div>
						</div>
						<FileImportView />
					</section>
					<FileInspectorView />
				</div>

				<div class="right-column-wrapper">
					<Show when={!runtime.settings.dialog().isOpen}>
						<SettingsPersistenceNotice />
					</Show>
					<RefusedChangeNotice />
					<div class="panel right-column-panel metadata-manager-panel">
						<MetadataManagerView />
					</div>
					<div class="panel right-column-panel output-and-tags-panel">
						<div class="output-and-tags-frame">
							<section
								class="output-and-tags"
								aria-label="Output and tags"
								data-testid="output-and-tags"
							>
								<div
									class="output-and-tags-block output-and-tags-block-output"
									data-testid="output-and-tags-output"
								>
									<OutputView />
								</div>
								<div
									class="output-and-tags-block output-and-tags-block-tags"
									data-testid="output-and-tags-tags"
								>
									<div class="output-and-tags-block-header tags-header">
										<h3>Tags Preview</h3>
										<PreviewAudioControls variant="compact" />
									</div>
									<TagPreviewView variant="workbench" />
								</div>
							</section>
						</div>
					</div>
					<StatusPanelView />
					<WorkCenterView />
				</div>
				<MetadataLookupView />
				<AppSettingsDialogView />
				<CollisionDialogView />
			</div>
		</Show>
	);
}
