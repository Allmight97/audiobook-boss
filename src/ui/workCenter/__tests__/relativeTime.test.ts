import { describe, expect, it } from 'vitest';
import { formatRelativeTime } from '../relativeTime';

describe('formatRelativeTime', () => {
	const nowMs = 10 * 24 * 60 * 60 * 1000;

	it('uses just now for elapsed times below 45 seconds', () => {
		expect(formatRelativeTime(nowMs - 44_999, nowMs)).toBe('just now');
	});

	it('uses whole minutes from 45 seconds until one hour', () => {
		expect(formatRelativeTime(nowMs - 45_000, nowMs)).toBe('1m ago');
		expect(formatRelativeTime(nowMs - 59 * 60 * 1000, nowMs)).toBe('59m ago');
	});

	it('uses whole hours from one hour until one day', () => {
		expect(formatRelativeTime(nowMs - 60 * 60 * 1000, nowMs)).toBe('1h ago');
		expect(formatRelativeTime(nowMs - 23 * 60 * 60 * 1000, nowMs)).toBe('23h ago');
	});

	it('uses whole days at one day and beyond', () => {
		expect(formatRelativeTime(nowMs - 24 * 60 * 60 * 1000, nowMs)).toBe('1d ago');
		expect(formatRelativeTime(nowMs - 3 * 24 * 60 * 60 * 1000, nowMs)).toBe('3d ago');
	});
});
