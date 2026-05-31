import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
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
	it('writes default run evidence outside repo-local .proof', () => {
		const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-proof-events-'));
		const writer = createArtifactWriter(repoRoot, 'review.main');
		try {
			writer.writeSummary(summary(writer.artifactDir, 'review.main'));

			expect(writer.artifactDir.startsWith(path.join(repoRoot, '.proof'))).toBe(false);
			expect(existsSync(path.join(repoRoot, '.proof'))).toBe(false);
			expect(existsSync(path.join(writer.artifactDir, 'summary.json'))).toBe(true);
		} finally {
			writer.discard();
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('supports injected artifact roots for focused tests', () => {
		const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-proof-events-'));
		const artifactRoot = path.join(repoRoot, 'proof-cache');
		try {
			const first = createArtifactWriter(repoRoot, 'focus.rust.lib.first', { artifactRoot });
			first.writeSummary(summary(first.artifactDir, 'focus.rust.lib.first'));

			const second = createArtifactWriter(repoRoot, 'review.main', { artifactRoot });
			second.writeSummary(summary(second.artifactDir, 'review.main'));

			expect(first.artifactDir).not.toBe(second.artifactDir);
			expect(first.artifactDir.startsWith(artifactRoot)).toBe(true);
			expect(existsSync(path.join(repoRoot, '.proof'))).toBe(false);
			expect(existsSync(path.join(first.artifactDir, 'summary.json'))).toBe(true);
			expect(existsSync(path.join(second.artifactDir, 'summary.json'))).toBe(true);
			expect(readFileSync(path.join(second.artifactDir, 'summary.json'), 'utf8')).toContain(
				'"id": "review.main"',
			);
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('keeps same-plan runs distinct even with the same timestamp prefix', () => {
		const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-proof-events-'));
		const artifactRoot = path.join(repoRoot, 'proof-cache');
		try {
			const timestamp = '2026-05-28T00:00:00.000Z';
			const first = createArtifactWriter(repoRoot, 'review.main', { artifactRoot, timestamp });
			first.writeSummary(summary(first.artifactDir, 'review.main'));

			const second = createArtifactWriter(repoRoot, 'review.main', { artifactRoot, timestamp });
			second.writeSummary(summary(second.artifactDir, 'review.main'));

			expect(first.artifactDir).not.toBe(second.artifactDir);
			expect(existsSync(path.join(first.artifactDir, 'summary.json'))).toBe(true);
			expect(existsSync(path.join(second.artifactDir, 'summary.json'))).toBe(true);
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('discards temporary run evidence after successful proof', () => {
		const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-proof-events-'));
		const artifactRoot = path.join(repoRoot, 'proof-cache');
		try {
			const writer = createArtifactWriter(repoRoot, 'review.main', { artifactRoot });
			writer.writeSummary(summary(writer.artifactDir, 'review.main'));

			expect(existsSync(writer.artifactDir)).toBe(true);

			writer.discard();

			expect(existsSync(writer.artifactDir)).toBe(false);
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});
});
