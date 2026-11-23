import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmodSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { describe, expect, it } from 'vitest';

const repo = path.resolve(import.meta.dirname, '..');
const head = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim();
const other = '0123456789abcdef0123456789abcdef01234567';
const version = execFileSync('bun', [path.join(repo, 'scripts', 'version-files.ts'), '--print'], {
	cwd: repo,
	encoding: 'utf8',
}).trim();
const tag = `v${version}`;
const product = JSON.parse(readFileSync(path.join(repo, 'src-tauri/tauri.conf.json'), 'utf8'))
	.productName as string;
const asset = `${product.replaceAll(' ', '.')}_${version}_aarch64.dmg`;
const checksum = `${asset}.sha256`;

interface Release {
	isDraft: boolean;
	assets: { name: string }[];
	targetCommitish: string;
}

interface GhState {
	tag: { type: string; sha: string } | null;
	release: Release | null;
}

const FAKE_GH = String.raw`#!/usr/bin/env bun
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
const args = process.argv.slice(2);
appendFileSync(process.env.FAKE_GH_LOG, args.join(' ') + '\n');
const state = JSON.parse(readFileSync(process.env.FAKE_GH_STATE, 'utf8'));
const save = () => writeFileSync(process.env.FAKE_GH_STATE, JSON.stringify(state));
const flag = (name) => args[args.indexOf(name) + 1];
const files = () => args.slice(3).filter((arg, i, all) => !arg.startsWith('--') && !all[i - 1]?.startsWith('--'));
const fail = (message) => { process.stderr.write(message + '\n'); process.exit(1); };
const [group, verb] = args;
if (group === 'api') {
	if (!args[1].includes('/git/ref/tags/') || !state.tag) fail('gh: Not Found (HTTP 404)');
	process.stdout.write(JSON.stringify(state.tag) + '\n');
} else if (group === 'release' && verb === 'view') {
	if (!state.release) fail('release not found');
	if (args.includes('url')) process.stdout.write('https://example.test/release\n');
	else process.stdout.write(JSON.stringify(state.release) + '\n');
} else if (group === 'release' && verb === 'create') {
	if (args.includes('--verify-tag') && !state.tag) fail('tag does not exist');
	const target = args.includes('--target') ? flag('--target') : state.tag.sha;
	state.release = { isDraft: true, assets: files().map((f) => ({ name: path.basename(f) })), targetCommitish: target };
	save();
} else if (group === 'release' && verb === 'upload') {
	state.release.assets.push(...files().map((f) => ({ name: path.basename(f) })));
	save();
} else if (group === 'release' && verb === 'edit') {
	state.release.isDraft = false;
	state.tag ??= { type: 'commit', sha: state.release.targetCommitish };
	save();
} else {
	fail('unexpected gh ' + args.join(' '));
}
`;

function tempDir(prefix: string): string {
	return mkdtempSync(path.join(tmpdir(), prefix));
}

function stagedAssets(sum?: string): string {
	const dir = tempDir('abb-stage-');
	writeFileSync(path.join(dir, asset), 'dmg-bytes');
	const digest = sum ?? createHash('sha256').update('dmg-bytes').digest('hex');
	writeFileSync(path.join(dir, checksum), `${digest}  ${asset}\n`);
	return dir;
}

function run(command: 'plan' | 'create', state: GhState, stage?: string) {
	const bin = tempDir('abb-gh-');
	writeFileSync(path.join(bin, 'gh'), FAKE_GH);
	chmodSync(path.join(bin, 'gh'), 0o755);
	const work = tempDir('abb-gh-state-');
	const stateFile = path.join(work, 'state.json');
	const log = path.join(work, 'calls.log');
	const output = path.join(work, 'output');
	writeFileSync(stateFile, JSON.stringify(state));
	writeFileSync(log, '');
	const result = spawnSync(
		'bash',
		['scripts/github-release.sh', command, ...(stage ? [stage] : [])],
		{
			cwd: repo,
			encoding: 'utf8',
			env: {
				...process.env,
				PATH: `${bin}:${process.env.PATH ?? ''}`,
				GITHUB_REPOSITORY: 'owner/repo',
				GITHUB_SHA: head,
				GITHUB_OUTPUT: output,
				GITHUB_EVENT_NAME: 'push',
				GITHUB_REF: 'refs/heads/main',
				FAKE_GH_STATE: stateFile,
				FAKE_GH_LOG: log,
			},
		},
	);
	const calls = readFileSync(log, 'utf8').split('\n').filter(Boolean);
	return {
		status: result.status,
		calls,
		stdout: result.stdout,
		stderr: result.stderr,
		output: existsSync(output) ? readFileSync(output, 'utf8') : '',
		writes: calls
			.filter((line) => /^release (create|upload|edit)/.test(line))
			.map((line) => line.split(' --')[0]),
		state: JSON.parse(readFileSync(stateFile, 'utf8')) as GhState,
	};
}

function release(isDraft: boolean, names: string[], target = head): Release {
	return { isDraft, assets: names.map((name) => ({ name })), targetCommitish: target };
}

const published = (target: string) => release(false, [asset, checksum], target);

describe('github-release.sh plan', () => {
	it('builds when neither the tag nor the release exists', () => {
		const result = run('plan', { tag: null, release: null });
		expect(result.status).toBe(0);
		expect(result.output).toBe('publish=true\n');
	});

	it('finishes a draft that targets this commit and has no tag yet', () => {
		const result = run('plan', { tag: null, release: release(true, []) });
		expect(result.status).toBe(0);
		expect(result.stdout).toContain('Finishing that draft');
		expect(result.output).toBe('publish=true\n');
	});

	it('refuses a draft that targets another commit', () => {
		const result = run('plan', { tag: null, release: release(true, [], other) });
		expect(result.status).toBe(1);
		expect(result.stderr).toContain('Fix: delete the draft, then rerun.');
	});

	it('skips a later push once the release is published from another commit', () => {
		const result = run('plan', { tag: { type: 'commit', sha: other }, release: published(other) });
		expect(result.status).toBe(0);
		expect(result.output).toBe('publish=false\n');
	});

	it('fails a later push while the release on another commit is unfinished', () => {
		const result = run('plan', {
			tag: { type: 'commit', sha: other },
			release: release(false, [asset], other),
		});
		expect(result.status).toBe(1);
		expect(result.stderr).toContain(`release-publish workflow of ${other}`);
	});

	it('fails a published release on this commit that is missing an asset', () => {
		const result = run('plan', {
			tag: { type: 'commit', sha: head },
			release: release(false, [asset]),
		});
		expect(result.status).toBe(1);
		expect(result.stderr).toContain(`missing ${checksum}`);
	});
});

describe('github-release.sh create', () => {
	it('creates a draft on this commit, then publishes it, which creates the tag', () => {
		const result = run('create', { tag: null, release: null }, stagedAssets());
		expect(result.status).toBe(0);
		expect(result.writes).toEqual([
			expect.stringMatching(new RegExp(`^release create ${tag} `)),
			`release edit ${tag}`,
		]);
		expect(result.state).toEqual({
			tag: { type: 'commit', sha: head },
			release: published(head),
		});
	});

	it('uploads only the missing asset of a draft for this commit, then publishes', () => {
		const result = run('create', { tag: null, release: release(true, [asset]) }, stagedAssets());
		expect(result.status).toBe(0);
		expect(result.writes).toEqual([
			expect.stringMatching(
				new RegExp(`^release upload ${tag} .*${checksum.replaceAll('.', '\\.')}$`),
			),
			`release edit ${tag}`,
		]);
		expect(result.state.release?.isDraft).toBe(false);
	});

	it('publishes against an existing tag on this commit without moving it', () => {
		const result = run(
			'create',
			{ tag: { type: 'commit', sha: head }, release: null },
			stagedAssets(),
		);
		expect(result.status).toBe(0);
		expect(result.writes[0]).toMatch(/^release create /);
		expect(result.calls).toContainEqual(expect.stringMatching(/^release create .*--verify-tag/));
		expect(result.state.tag).toEqual({ type: 'commit', sha: head });
	});

	it('writes nothing when the staged checksum does not match the DMG', () => {
		const result = run('create', { tag: null, release: null }, stagedAssets('0'.repeat(64)));
		expect(result.status).toBe(1);
		expect(result.stderr).toContain('does not match the DMG');
		expect(result.writes).toEqual([]);
	});

	it('writes nothing once the release is published from another commit', () => {
		const result = run(
			'create',
			{ tag: { type: 'commit', sha: other }, release: published(other) },
			stagedAssets(),
		);
		expect(result.status).toBe(0);
		expect(result.writes).toEqual([]);
	});
});
