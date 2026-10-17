import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { checkWorkflow } from './check-pr-workflow-policy';

const repoRoot = path.resolve(import.meta.dirname, '..');

function messages(file: string, text: string): string[] {
	return checkWorkflow(file, text).map((finding) => `${finding.line}: ${finding.message}`);
}

const guarded = `
on:
  pull_request:
    types: [opened, synchronize, reopened, ready_for_review, converted_to_draft, closed]
concurrency:
  group: \${{ github.workflow }}-\${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true
jobs:
  frontend:
    if: \${{ github.event.pull_request.draft != true }}
    runs-on: ubuntu-24.04
    timeout-minutes: 5
`;

describe('check-pr-workflow-policy', () => {
	// Fixture is 64ec2240^:.github/workflows/ci.yml, before drafts were skipped.
	it('fails the draft leak in the vendored pre-guard workflow', () => {
		const text = readFileSync(
			path.join(repoRoot, 'scripts/fixtures/ci-64ec2240-parent.yml'),
			'utf8',
		);
		const found = messages('.github/workflows/ci.yml', text);
		expect(found.length).toBeGreaterThan(0);
		expect(found.join('\n')).toMatch(/job frontend runs on a draft pull request/);
	});

	it('rejects a scalar pull_request trigger', () => {
		const found = messages(
			'ci.yml',
			'on: pull_request\njobs:\n  frontend:\n    runs-on: ubuntu-24.04\n    timeout-minutes: 5\n',
		);
		expect(found.join('\n')).toMatch(/pull_request types are missing/);
		expect(found.join('\n')).toMatch(/job frontend runs on a draft pull request/);
	});

	it('rejects an array pull_request trigger', () => {
		const found = messages(
			'ci.yml',
			'on: [pull_request]\njobs:\n  frontend:\n    runs-on: ubuntu-24.04\n    timeout-minutes: 5\n',
		);
		expect(found.join('\n')).toMatch(/pull_request types are missing/);
		expect(found.join('\n')).toMatch(/job frontend runs on a draft pull request/);
	});

	it('rejects a draft guard ored with true', () => {
		const found = messages(
			'ci.yml',
			guarded.replace(
				'github.event.pull_request.draft != true',
				'github.event.pull_request.draft != true || true',
			),
		);
		expect(found.join('\n')).toMatch(/bypasses its draft guard/);
	});

	it('rejects ubuntu-latest inside a matrix the job runs on', () => {
		const text = guarded.replace(
			'runs-on: ubuntu-24.04',
			`runs-on: \${{ matrix.os }}\n    strategy:\n      matrix:\n        os: [ubuntu-latest]`,
		);
		const found = messages('ci.yml', text);
		expect(found.join('\n')).toMatch(/job frontend uses ubuntu-latest/);
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
