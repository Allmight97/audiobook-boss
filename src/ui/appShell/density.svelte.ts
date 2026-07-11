import type { DensityPreference } from '../../types/appSettings';
import { persistAppSettingsPatch } from '../appSettings';

export const densityState = $state({ preference: 'comfortable' as DensityPreference });

export function applyDensityPreference(preference: DensityPreference): void {
	densityState.preference = preference;
	if (typeof document === 'undefined') {
		return;
	}

	if (preference === 'compact') {
		document.documentElement.dataset.density = 'compact';
	} else {
		delete document.documentElement.dataset.density;
	}
}

export function setDensityFromUser(preference: DensityPreference): void {
	applyDensityPreference(preference);
	void persistAppSettingsPatch({ density: preference });
}
