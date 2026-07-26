import type { EditSurfacePreference } from '../../types/appSettings';
import { persistAppSettingsPatch } from '../appSettings';

/** The metadata edit surface presentation: rail (v3 default) or popover. */
export const editSurfaceState = $state({ preference: 'rail' as EditSurfacePreference });

export function applyEditSurfacePreference(preference: EditSurfacePreference): void {
	editSurfaceState.preference = preference;
}

export function setEditSurfaceFromUser(preference: EditSurfacePreference): void {
	applyEditSurfacePreference(preference);
	void persistAppSettingsPatch({ editSurface: preference });
}
