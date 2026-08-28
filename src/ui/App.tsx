import type { JSX } from 'solid-js';
import { FileImportView } from './fileImport/FileImportView';
import { MergeModeToggle } from './jobControls/MergeModeToggle';
import { FileInspectorView } from './leftColumn/FileInspectorView';

export function App(): JSX.Element {
	return (
		<div class="main-container">
			<div class="panel input-panel left-column-wrapper" data-testid="left-column">
				<section
					class="left-column-panel input-workflow input-workflow-panel flex flex-col gap-2 mb-2"
					data-testid="input-workflow-panel"
					aria-label="Input and File Order"
				>
					<div class="flex items-center justify-between">
						<h3 class="section-title mb-0 whitespace-nowrap mr-2">Input and File Order</h3>
						<MergeModeToggle />
					</div>
					<FileImportView />
				</section>
				<FileInspectorView />
			</div>
		</div>
	);
}
