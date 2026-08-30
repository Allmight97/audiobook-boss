import type { JSX } from 'solid-js';
import { useAppRuntime } from '../../app/runtime';

export function MergeModeToggle(): JSX.Element {
	const input = useAppRuntime().input;
	const jobType = input.jobType;
	const setJobType = input.setJobType;

	return (
		<div class="flex items-center gap-1.5">
			<div class="relative group">
				<div class="info-icon">i</div>
				<div class="info-popover">
					Combines all files in the list into a single audiobook in the order they appear. Each file
					will be treated as a chapter.
				</div>
			</div>
			<label class="checkbox-label text-xs mb-0" data-testid="merge-toggle">
				<input
					type="checkbox"
					id="merge-mode-toggle"
					checked={jobType() === 'merge'}
					onChange={(event) => setJobType(event.currentTarget.checked ? 'merge' : 'batch')}
				/>
				<span class="option-label">Merge files into one audiobook</span>
			</label>
		</div>
	);
}
