import type { ConcurrencySnapshot } from '../generated/tauri';

export function concurrencyLabel({ preference, effective }: ConcurrencySnapshot): string {
	return `Max jobs: ${effective}${preference.mode === 'auto' ? ' (Auto)' : ''}`;
}
