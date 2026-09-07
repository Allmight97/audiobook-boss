import type { JSX } from '@solidjs/web';

import { CUSTOM_TEMPLATE_PLACEHOLDER } from '../../app/outputPlan';
import { useAppRuntime } from '../../app/runtime';
import './outputView.css';

export function OutputView(): JSX.Element {
	const output = useAppRuntime().output;
	const view = output.view;
	const browse = output.browseDirectory;
	const selectPreset = output.selectNamingPreset;
	const setYear = output.setAbsIncludeYear;
	const editTemplate = output.editNamingTemplate;

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
						class="output-browse-button"
						type="button"
						onClick={() => void browse()}
					>
						Browse…
					</button>
				</div>

				<div class="output-options-panel">
					<div class="path-option-row">
						<label for="output-naming-preset" class="output-option-label">
							Naming preset
						</label>
						<select
							id="output-naming-preset"
							class="output-preset-select"
							value={view().namingPreset}
							onChange={(event) => selectPreset(event.currentTarget.value)}
						>
							<option value="absDefault">ABS Default</option>
							<option value="customTemplate">Custom Template</option>
						</select>
						<div class="group">
							<div class="info-icon">i</div>
							<div class="info-popover">
								ABS Default keeps Audiobookshelf-compatible paths. Custom Template stores your draft
								template.
							</div>
						</div>
					</div>
					<div class="path-option-row" id="output-template-row" hidden={view().templateRowHidden}>
						<label for="output-template-input" class="output-option-label">
							Template
						</label>
						<input
							id="output-template-input"
							type="text"
							class="output-template-input"
							value={view().namingTemplate}
							placeholder={CUSTOM_TEMPLATE_PLACEHOLDER}
							onInput={(event) => editTemplate(event.currentTarget.value)}
							autocomplete="off"
							spellcheck={false}
						/>
					</div>
					<div class="path-option-row" id="output-abs-options">
						<label class="checkbox-label tight">
							<input
								type="checkbox"
								id="output-abs-include-year"
								checked={view().absIncludeYear}
								onChange={(event) => setYear(event.currentTarget.checked)}
							/>
							Include year segment (YYYY)
						</label>
						<span id="output-abs-hint" class="muted-text" hidden={view().absHintHidden}>
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
