import { describe, expect, it } from 'vitest';

import { parseMetadataStageLine, renderMetadataStages } from './dev-log-metadata';

describe('dev-log-metadata', () => {
	it('ignores stage starts and parses completed metadata stage metrics', () => {
		expect(
			parseMetadataStageLine('media_stage stage=metadata_replace status=start artifact=abc'),
		).toBeNull();
		expect(
			parseMetadataStageLine('media_stage stage=encode_mux status=ok artifact=abc elapsed_ms=10'),
		).toBeNull();

		const parsed = parseMetadataStageLine(
			'[2026-11-27T12:00:00Z INFO abb_media_pipeline] media_stage stage=metadata_replace status=ok artifact=a1 elapsed_ms=280 sync_ms=250 chmod_ms=4 rename_ms=12',
		);
		expect(parsed).toEqual({
			stage: 'metadata_replace',
			status: 'ok',
			artifact: 'a1',
			elapsedMs: 280,
			syncMs: 250,
			chmodMs: 4,
			renameMs: 12,
		});
	});

	it('renders a compact table with strategy, fs, layout shift, and sync timings', () => {
		const stages = [
			parseMetadataStageLine(
				'media_stage stage=metadata_copy_source status=ok artifact=a1 elapsed_ms=5 strategy=cow_clone fs=apfs bytes_written=1024',
			),
			parseMetadataStageLine(
				'media_stage stage=metadata_mp4_tags status=ok artifact=a1 elapsed_ms=12 layout_before=moov_front layout_after=moov_end mdat_shifted=false close_ms=3 bytes_written=4096',
			),
			parseMetadataStageLine(
				'media_stage stage=metadata_verify_audio status=error artifact=a1 elapsed_ms=4 mode=structural bytes_read=8192',
			),
		].filter((stage): stage is NonNullable<typeof stage> => stage !== null);

		const rendered = renderMetadataStages(stages).join('\n');
		expect(rendered).toContain(
			'| metadata_copy_source | `a1` | ok | 5ms | strategy=cow_clone fs=apfs written=1024 |',
		);
		expect(rendered).toContain(
			'layout=moov_front->moov_end mdat_shifted=false close=3ms written=4096',
		);
		expect(rendered).toContain(
			'| metadata_verify_audio | `a1` | error | 4ms | mode=structural read=8192 |',
		);
	});
});
