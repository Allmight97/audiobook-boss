import { persistAppSettingsPatch } from '../appSettings';

// Mirrors the backend clamp in `src-tauri/src/app_settings/types.rs`
// (RAIL_WIDTH_MIN/MAX); the backend remains the authority on persisted values.
export const RAIL_WIDTH_MIN = 340;
export const RAIL_WIDTH_MAX = 640;
export const RAIL_WIDTH_DEFAULT = 420;
export const RAIL_WIDTH_KEYBOARD_STEP = 16;

const railWidthState = $state({ width: RAIL_WIDTH_DEFAULT });

function clampRailWidth(width: number): number {
	if (!Number.isFinite(width)) return RAIL_WIDTH_DEFAULT;
	return Math.round(Math.min(RAIL_WIDTH_MAX, Math.max(RAIL_WIDTH_MIN, width)));
}

export function readRailWidth(): number {
	return railWidthState.width;
}

/** Hydration applier — never persists (see appSettings AGENTS). */
export function applyRailWidthPreference(width: number): void {
	railWidthState.width = clampRailWidth(width);
}

/** Live width during a drag; nothing is persisted until the drag commits. */
export function previewRailWidthFromUser(width: number): void {
	railWidthState.width = clampRailWidth(width);
}

/** Commit a user-chosen width: apply and persist the clamped value. */
export function setRailWidthFromUser(width: number): void {
	applyRailWidthPreference(width);
	void persistAppSettingsPatch({ railWidth: railWidthState.width });
}
