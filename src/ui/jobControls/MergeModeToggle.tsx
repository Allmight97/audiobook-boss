import type { JSX } from '@solidjs/web';

import { useAppRuntime } from '../../app/runtime';
import './jobControls.css';

export function MergeModeToggle(): JSX.Element {
	const input = useAppRuntime().input;
	const jobType = input.jobType;
	const setJobType = input.setJobType;

	return (
		<div class="merge-mode">
			<div class="group">
				<div class="info-icon">i</div>
				<div class="info-popover">
					Combines all files in the list into a single audiobook in the order they appear. Each file
					will be treated as a chapter.
				</div>
			</div>
			<label class="checkbox-label tight" data-testid="merge-toggle">
				<input
					type="checkbox"
					id="merge-mode-toggle"
					checked={jobType() === 'merge'}
					disabled={input.view().orderLocked}
					onChange={(event) => setJobType(event.currentTarget.checked ? 'merge' : 'batch')}
				/>
				<span class="option-label">Merge files into one audiobook</span>
			</label>
		</div>
	);
}
