import type { JSX } from 'solid-js';
import {
	browseOutputDirectoryAtom,
	CUSTOM_TEMPLATE_PLACEHOLDER,
	editNamingTemplateAtom,
	outputPathPreviewAtom,
	outputViewAtom,
	selectNamingPresetAtom,
	setAbsIncludeYearAtom,
} from '../../app/outputPlan';
import { useAtomSet, useAtomValue } from '../../app/runtime/solid';
import './outputView.css';

export function OutputView(): JSX.Element {
	const view = useAtomValue(() => outputViewAtom);
	useAtomValue(() => outputPathPreviewAtom);
	const browse = useAtomSet(() => browseOutputDirectoryAtom);
	const selectPreset = useAtomSet(() => selectNamingPresetAtom);
	const setYear = useAtomSet(() => setAbsIncludeYearAtom);
	const editTemplate = useAtomSet(() => editNamingTemplateAtom);

	return (
		<div class="output-panel output-panel-workbench" data-testid="output-panel">
			<div class="section-header">
				<h3>Output</h3>
			</div>
			<div class="output-panel-body">
				<label for="output-dir-browse" class="output-subtitle">
					Output Directory
				</label>
				<div class="output-preview-box">
					<div
						id="output-dir-text"
						class="output-path-text"
						title={view().outputDirectory || view().previewTitle}
						data-testid="output-directory-value"
					>
						{view().displayDirectory}
					</div>
					<button
						id="output-dir-browse"
						class="btn-pill btn-pill-primary-soft output-browse-button"
						type="button"
						onClick={() => void browse(undefined)}
					>
						Browse…
					</button>
				</div>

				<div class="output-options-panel">
					<div class="path-option-row flex items-center gap-2">
						<label for="output-naming-preset" class="text-xs mt-0">
							Naming preset
						</label>
						<select
							id="output-naming-preset"
							class="w-auto min-w-40"
							value={view().namingPreset}
							onChange={(event) => selectPreset(event.currentTarget.value)}
						>
							<option value="absDefault">ABS Default</option>
							<option value="customTemplate">Custom Template</option>
						</select>
						<div class="relative group">
							<div class="info-icon">i</div>
							<div class="info-popover">
								ABS Default keeps Audiobookshelf-compatible paths. Custom Template stores your draft
								template.
							</div>
						</div>
					</div>
					<div class="path-option-row" id="output-template-row" hidden={view().templateRowHidden}>
						<label for="output-template-input" class="text-xs mt-0">
							Template
						</label>
						<input
							id="output-template-input"
							type="text"
							class="w-full"
							value={view().namingTemplate}
							placeholder={CUSTOM_TEMPLATE_PLACEHOLDER}
							onInput={(event) => editTemplate(event.currentTarget.value)}
							autocomplete="off"
							spellcheck={false}
						/>
					</div>
					<div class="path-option-row" id="output-abs-options">
						<label class="checkbox-label text-xs mt-0">
							<input
								type="checkbox"
								id="output-abs-include-year"
								checked={view().absIncludeYear}
								onChange={(event) => setYear(event.currentTarget.checked)}
							/>
							Include year segment (YYYY)
						</label>
						<span id="output-abs-hint" class="text-xs muted-text" hidden={view().absHintHidden}>
							{view().absHintText}
						</span>
					</div>
					<div class="output-example" data-testid="output-example">
						<span class="output-example-label">Example:</span>
						<span class="output-example-path" title={view().previewTitle}>
							{view().previewText}
						</span>
					</div>
				</div>
			</div>
		</div>
	);
}
