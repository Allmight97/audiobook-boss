import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { checkWorkflow } from './check-pr-workflow-policy';

const repoRoot = path.resolve(import.meta.dirname, '..');

function messages(file: string, text: string): string[] {
	return checkWorkflow(file, text).map((finding) => `${finding.line}: ${finding.message}`);
}

describe('check-pr-workflow-policy', () => {
	// 64ec2240^:.github/workflows/ci.yml ran the frontend job on draft PRs.
	it('fails the draft leak in ci.yml before drafts were skipped', () => {
		const text = execFileSync('git', ['show', '64ec2240^:.github/workflows/ci.yml'], {
			cwd: repoRoot,
			encoding: 'utf8',
		});
		const found = messages('.github/workflows/ci.yml', text);
		expect(found.length).toBeGreaterThan(0);
		expect(found.join('\n')).toMatch(/job frontend runs on a draft pull request/);
	});

	it('accepts the workflows in this checkout', () => {
		const files = readdirSync(path.join(repoRoot, '.github/workflows')).filter((name) =>
			name.endsWith('.yml'),
		);
		const found = files.flatMap((name) => {
			const file = path.join('.github/workflows', name);
			return messages(file, readFileSync(path.join(repoRoot, file), 'utf8'));
		});
		expect(found).toEqual([]);
	});
});
