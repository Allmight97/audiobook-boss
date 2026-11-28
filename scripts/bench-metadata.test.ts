import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import {
	cleanupScratchRoots,
	compareAudioHashes,
	hasBenchmarkFailure,
	inspectMp4File,
} from './bench-metadata';

function box(type: string, payload: Buffer): Buffer {
	const out = Buffer.alloc(8 + payload.length);
	out.writeUInt32BE(out.length, 0);
	out.write(type, 4, 4, 'latin1');
	payload.copy(out, 8);
	return out;
}

describe('inspectMp4File', () => {
	it('reports front moov layout, free padding bytes, and mdat hash', () => {
		const dir = path.join(tmpdir(), `abb-bench-test-${process.pid}`);
		const file = path.join(dir, 'front.m4b');
		const bytes = Buffer.concat([
			box('ftyp', Buffer.from('M4A \0\0\0\0')),
			box('moov', Buffer.alloc(16, 1)),
			box('free', Buffer.alloc(64, 0)),
			box('mdat', Buffer.from('audio-payload-bytes')),
		]);
		mkdirSync(dir, { recursive: true });
		try {
			writeFileSync(file, bytes);
			const result = inspectMp4File(file);
			expect(result?.layout).toBe('moov_front');
			expect(result?.freeBytes).toBe(72);
			expect(result?.mdatSha256).toHaveLength(64);
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('flags nonzero exit codes and audio digest mismatches while cleaning scratch dirs', () => {
		expect(compareAudioHashes('abc', 'abc')).toBe(true);
		expect(compareAudioHashes('abc', null)).toBe(false);
		expect(compareAudioHashes(null, null)).toBeNull();
		expect(
			hasBenchmarkFailure([
				{ rc: 0, audioHashMatch: compareAudioHashes(null, null) },
				{ rc: 0, audioHashMatch: true },
			]),
		).toBe(false);
		expect(hasBenchmarkFailure([{ rc: 0, audioHashMatch: compareAudioHashes('abc', null) }])).toBe(
			true,
		);
		expect(hasBenchmarkFailure([{ rc: 1, audioHashMatch: true }])).toBe(true);
		expect(hasBenchmarkFailure([{ rc: 0, audioHashMatch: false }])).toBe(true);

		const scratch = path.join(tmpdir(), `abb-bench-clean-${process.pid}`);
		mkdirSync(scratch, { recursive: true });
		writeFileSync(path.join(scratch, 'copy.m4b'), 'bytes');
		cleanupScratchRoots([scratch]);
		expect(existsSync(scratch)).toBe(false);
	});
});
