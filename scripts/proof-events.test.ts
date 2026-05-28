import { existsSync, mkdtempSync, readFileSync, readlinkSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { createArtifactWriter } from './proof/events';
import type { ProofSummary } from './proof/types';

function summary(artifactDir: string, id: string): ProofSummary {
	return {
		artifactDir,
		durationMs: 12,
		plan: {
			classification: 'focused',
			id,
			label: id,
			purpose: 'test fixture',
		},
		status: 'passed',
		steps: [],
	};
}

describe('proof artifact writer', () => {
	it('keeps immutable run directories and points latest at the newest run', () => {
		const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-proof-events-'));
		try {
			const first = createArtifactWriter(repoRoot, 'focus.rust.lib.first');
			first.writeSummary(summary(first.artifactDir, 'focus.rust.lib.first'));

			const second = createArtifactWriter(repoRoot, 'review.main');
			second.writeSummary(summary(second.artifactDir, 'review.main'));

			expect(first.artifactDir).not.toBe(second.artifactDir);
			expect(existsSync(path.join(first.artifactDir, 'summary.json'))).toBe(true);
			expect(existsSync(path.join(second.artifactDir, 'summary.json'))).toBe(true);
			expect(readlinkSync(path.join(repoRoot, '.proof', 'latest'))).toBe(
				path.relative(path.join(repoRoot, '.proof'), second.artifactDir),
			);
			expect(
				readFileSync(path.join(repoRoot, '.proof', 'latest', 'summary.json'), 'utf8'),
			).toContain('"id": "review.main"');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('keeps same-plan runs distinct even with the same timestamp prefix', () => {
		const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-proof-events-'));
		try {
			const timestamp = '2026-05-28T00:00:00.000Z';
			const first = createArtifactWriter(repoRoot, 'review.main', { timestamp });
			first.writeSummary(summary(first.artifactDir, 'review.main'));

			const second = createArtifactWriter(repoRoot, 'review.main', { timestamp });
			second.writeSummary(summary(second.artifactDir, 'review.main'));

			expect(first.artifactDir).not.toBe(second.artifactDir);
			expect(readlinkSync(path.join(repoRoot, '.proof', 'latest'))).toBe(
				path.relative(path.join(repoRoot, '.proof'), second.artifactDir),
			);
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});
});
