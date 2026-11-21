import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	appendFileSync,
	existsSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { predictedReleaseAssetName } from './resolve-release-dmg';
import {
	changelogReleaseNotes,
	readVersionRelease,
	versionReleaseFindings,
	type VersionSurface,
} from './version-files';

const HELP = `Usage: bun scripts/github-publish.ts <plan|create> [staged-dir]

plan    Decide whether this push to main should build a release.
create  Tag vX.Y.Z when it is absent and publish the GitHub Release for the
        staged DMG and its SHA-256. A second run does not move the tag or
        publish a second release. Refuses unless this is a push to main.`;

export interface RemoteRelease {
	draft: boolean;
	assetNames: string[];
}

export interface ReleaseObservation {
	tag: string;
	headSha: string;
	tagSha: string | null;
	release: RemoteRelease | null;
	dmgAsset: string;
	checksumAsset: string;
}

export type ReleasePlan =
	| { action: 'skip'; message: string }
	| { action: 'build'; createTag: boolean; message: string }
	| { action: 'refuse'; message: string };

export function planGitHubRelease(observed: ReleaseObservation): ReleasePlan {
	const { tag, tagSha, release, dmgAsset, checksumAsset } = observed;
	const head = observed.headSha;
	const names = new Set(release?.assetNames ?? []);
	const published = release !== null && !release.draft;
	const complete = published && names.has(dmgAsset) && names.has(checksumAsset);
	if (tagSha && tagSha.toLowerCase() !== head.toLowerCase()) {
		if (published && !complete) {
			return {
				action: 'skip',
				message: `Tag ${tag} points at ${tagSha}, not this commit, and the published release is missing assets. Not tagging or publishing. Fix: delete that release by hand, then rerun the workflow of ${tagSha}.`,
			};
		}
		return {
			action: 'skip',
			message: complete
				? `${tag} is already published from ${tagSha}. Not building.`
				: `Tag ${tag} points at ${tagSha}, not ${head}. Not tagging or publishing. Rerun the workflow of ${tagSha} if that release did not finish.`,
		};
	}
	if (complete) {
		return {
			action: 'skip',
			message: `${tag} is already published with its DMG and SHA-256. Not building.`,
		};
	}
	if (published) {
		const missing = [dmgAsset, checksumAsset].filter((name) => !names.has(name));
		return {
			action: 'refuse',
			message: `Published release ${tag} is immutable and is missing ${missing.join(', ')}. Fix: delete that release by hand, then rerun. This workflow will not publish a second release.`,
		};
	}
	if (release?.draft && !tagSha) {
		return {
			action: 'refuse',
			message: `A draft ${tag} exists without the tag. Fix: delete the draft, then rerun.`,
		};
	}
	return {
		action: 'build',
		createTag: tagSha === null,
		message: tagSha
			? `Tag ${tag} is on this commit and the release is not published. Building the DMG and finishing the release.`
			: `No tag ${tag}. Building the DMG, then tagging this commit and publishing.`,
	};
}

interface CommandResult {
	status: number;
	stdout: string;
	stderr: string;
}

interface TagRef {
	name: string;
	type: string;
	sha: string;
}

function runGh(args: string[]): CommandResult {
	const result = spawnSync('gh', args, { encoding: 'utf8' });
	if (result.error) throw result.error;
	return { status: result.status ?? 1, stdout: result.stdout, stderr: result.stderr.trim() };
}

function gh(args: string[]): string {
	const result = runGh(args);
	if (result.status !== 0) {
		throw new Error(`'gh ${args[0]}' exited ${result.status}: ${result.stderr}`);
	}
	return result.stdout.trim();
}

function repository(): string {
	const repo = process.env.GITHUB_REPOSITORY ?? '';
	if (!/^[^/\s]+\/[^/\s]+$/.test(repo)) {
		throw new Error('GITHUB_REPOSITORY is not set. Fix: run this step in GitHub Actions.');
	}
	return repo;
}

function readHeadSha(repoRoot: string): string {
	const sha = exec('git', ['rev-parse', 'HEAD'], repoRoot);
	if (!/^[0-9a-f]{40}$/.test(sha)) {
		throw new Error(
			`HEAD is not a full commit SHA ('${sha}'). Fix: check out the commit to release.`,
		);
	}
	const expected = process.env.GITHUB_SHA;
	if (expected && expected !== sha) {
		throw new Error(
			`Checkout ${sha} is not the workflow commit ${expected}. Fix: rerun the workflow so it builds the commit it tags.`,
		);
	}
	return sha;
}

function exec(command: string, args: string[], cwd: string): string {
	const result = spawnSync(command, args, { cwd, encoding: 'utf8' });
	if (result.error) throw result.error;
	if ((result.status ?? 1) !== 0) {
		throw new Error(
			`'${command} ${args.join(' ')}' exited ${result.status}: ${result.stderr.trim()}`,
		);
	}
	return result.stdout.trim();
}

function listTags(repo: string): TagRef[] {
	const stdout = gh([
		'api',
		'--paginate',
		`repos/${repo}/git/matching-refs/tags/v`,
		'--jq',
		'.[] | {ref: .ref, type: .object.type, sha: .object.sha}',
	]);
	return stdout
		.split('\n')
		.map((line) => line.trim())
		.filter((line) => line.length > 0)
		.map((line) => JSON.parse(line) as { ref: string; type: string; sha: string })
		.map((ref) => ({ name: ref.ref.replace('refs/tags/', ''), type: ref.type, sha: ref.sha }));
}

function commitOfTag(repo: string, ref: TagRef): string {
	if (ref.type === 'commit') return ref.sha;
	if (ref.type !== 'tag') {
		throw new Error(`Tag ${ref.name} points at a ${ref.type}, not a commit.`);
	}
	const sha = gh(['api', `repos/${repo}/git/tags/${ref.sha}`, '--jq', '.object.sha']);
	if (!/^[0-9a-f]{40}$/.test(sha)) {
		throw new Error(`Tag ${ref.name} did not resolve to a commit.`);
	}
	return sha;
}

function viewRelease(repo: string, tag: string): RemoteRelease | null {
	const result = runGh(['release', 'view', tag, '--repo', repo, '--json', 'isDraft,assets']);
	if (result.status !== 0) {
		if (/not found/i.test(result.stderr)) return null;
		throw new Error(`Cannot read GitHub Release ${tag}: ${result.stderr}`);
	}
	const parsed = JSON.parse(result.stdout) as {
		isDraft: boolean;
		assets: { name: string }[];
	};
	return { draft: parsed.isDraft, assetNames: parsed.assets.map((asset) => asset.name) };
}

function productName(repoRoot: string): string {
	const config = JSON.parse(
		readFileSync(path.join(repoRoot, 'src-tauri/tauri.conf.json'), 'utf8'),
	) as { productName?: unknown };
	if (typeof config.productName !== 'string' || config.productName.length === 0) {
		throw new Error('src-tauri/tauri.conf.json has no productName.');
	}
	return config.productName;
}

function agreedVersion(versions: VersionSurface[]): string {
	const version = versions[0]?.version;
	if (!version) {
		throw new Error(
			'Version files disagree. Fix: set every version file to the same x.y.z with `bun scripts/bump-version.ts <x.y.z>`.',
		);
	}
	return version;
}

function observe(repoRoot: string, repo: string): ReleaseObservation {
	const input = readVersionRelease(repoRoot);
	const version = agreedVersion(input.versions);
	const tag = `v${version}`;
	const tags = listTags(repo);
	const findings = versionReleaseFindings({ ...input, tags: tags.map((ref) => ref.name) });
	if (findings.length > 0) {
		throw new Error(findings.map((finding) => finding.message).join('\n'));
	}
	const dmgAsset = predictedReleaseAssetName(productName(repoRoot), version);
	const match = tags.find((ref) => ref.name === tag);
	return {
		tag,
		headSha: readHeadSha(repoRoot),
		tagSha: match ? commitOfTag(repo, match) : null,
		release: viewRelease(repo, tag),
		dmgAsset,
		checksumAsset: `${dmgAsset}.sha256`,
	};
}

function releaseNotes(repoRoot: string, tag: string): string {
	const version = tag.slice(1);
	const notes = changelogReleaseNotes(readVersionRelease(repoRoot).changelog, version);
	if (!notes) {
		throw new Error(
			`CHANGELOG.md has no non-empty '## [${version}]' section. Fix: rename '## [Unreleased]' to '## [${version}] - YYYY-MM-DD' and write the user-facing notes under that heading.`,
		);
	}
	return notes;
}

function setOutput(name: string, value: string): void {
	const file = process.env.GITHUB_OUTPUT;
	if (!file) return;
	appendFileSync(file, `${name}=${value}\n`);
}

function assertPushToMain(): void {
	if (process.env.GITHUB_EVENT_NAME !== 'push' || process.env.GITHUB_REF !== 'refs/heads/main') {
		throw new Error(
			'Refusing to tag or publish outside a push to main. Fix: merge a pull request that bumps the version; that push publishes the release.',
		);
	}
}

function createDraft(
	repo: string,
	observed: ReleaseObservation,
	createTag: boolean,
	dmg: string,
	checksum: string,
	notesFile: string,
): void {
	const args = [
		'release',
		'create',
		observed.tag,
		dmg,
		checksum,
		'--repo',
		repo,
		'--draft',
		'--title',
		`AudioBook Boss ${observed.tag}`,
		'--notes-file',
		notesFile,
	];
	if (createTag) args.push('--target', observed.headSha);
	else args.push('--verify-tag');
	gh(args);
}

function uploadMissing(repo: string, tag: string, files: string[], release: RemoteRelease): void {
	const missing = files.filter((file) => !release.assetNames.includes(path.basename(file)));
	if (missing.length === 0) return;
	gh(['release', 'upload', tag, ...missing, '--repo', repo]);
}

function stagedAssets(
	dir: string,
	observed: ReleaseObservation,
): { dmg: string; checksum: string; sha256: string } {
	const dmg = path.join(dir, observed.dmgAsset);
	const checksum = path.join(dir, observed.checksumAsset);
	if (!existsSync(dmg) || !existsSync(checksum)) {
		throw new Error(
			`Staged assets ${observed.dmgAsset} and ${observed.checksumAsset} are missing in ${dir}. Fix: build and stage the DMG before publishing.`,
		);
	}
	const sha256 = createHash('sha256').update(readFileSync(dmg)).digest('hex');
	const recorded = readFileSync(checksum, 'utf8').trim().split(/\s+/)[0];
	if (recorded !== sha256) {
		throw new Error(
			`Staged checksum ${recorded} does not match the DMG ${sha256}. Fix: stage the DMG again; the published file must be the one that was checked.`,
		);
	}
	return { dmg, checksum, sha256 };
}

function finishRelease(
	repo: string,
	observed: ReleaseObservation,
	createTag: boolean,
	dmg: string,
	checksum: string,
	notes: string,
): void {
	const notesDir = mkdtempSync(path.join(os.tmpdir(), 'abb-release-notes-'));
	try {
		const notesFile = path.join(notesDir, 'notes.md');
		writeFileSync(notesFile, notes);
		if (!observed.release) {
			createDraft(repo, observed, createTag, dmg, checksum, notesFile);
		}
		const draft = viewRelease(repo, observed.tag);
		if (!draft) {
			throw new Error(`GitHub Release ${observed.tag} was not created. Fix: rerun this workflow.`);
		}
		if (draft.draft) {
			uploadMissing(repo, observed.tag, [dmg, checksum], draft);
			const attached = viewRelease(repo, observed.tag);
			const names = new Set(attached?.assetNames ?? []);
			const missing = [observed.dmgAsset, observed.checksumAsset].filter(
				(name) => !names.has(name),
			);
			if (missing.length > 0) {
				throw new Error(
					`Draft ${observed.tag} is missing ${missing.join(', ')}. Fix: rerun this workflow; it will not create a second release.`,
				);
			}
			gh([
				'release',
				'edit',
				observed.tag,
				'--repo',
				repo,
				'--draft=false',
				'--title',
				`AudioBook Boss ${observed.tag}`,
				'--notes-file',
				notesFile,
			]);
		}
	} finally {
		rmSync(notesDir, { recursive: true, force: true });
	}
	const published = viewRelease(repo, observed.tag);
	const names = new Set(published?.assetNames ?? []);
	if (
		!published ||
		published.draft ||
		!names.has(observed.dmgAsset) ||
		!names.has(observed.checksumAsset)
	) {
		throw new Error(
			`GitHub Release ${observed.tag} is not published with its DMG and SHA-256. Fix: rerun this workflow; it will not create a second release.`,
		);
	}
	const url = gh([
		'release',
		'view',
		observed.tag,
		'--repo',
		repo,
		'--json',
		'url',
		'--jq',
		'.url',
	]);
	console.log(`GitHub Release ${url}`);
}

function plan(repoRoot: string): void {
	const observed = observe(repoRoot, repository());
	const decision = planGitHubRelease(observed);
	console.log(decision.message);
	if (decision.action === 'refuse') throw new Error(decision.message);
	if (decision.action === 'build') releaseNotes(repoRoot, observed.tag);
	setOutput('publish', decision.action === 'build' ? 'true' : 'false');
}

function publish(repoRoot: string, dir: string): void {
	assertPushToMain();
	const repo = repository();
	const observed = observe(repoRoot, repo);
	const decision = planGitHubRelease(observed);
	console.log(decision.message);
	if (decision.action === 'skip') return;
	if (decision.action === 'refuse') throw new Error(decision.message);
	const notes = releaseNotes(repoRoot, observed.tag);
	const staged = stagedAssets(dir, observed);
	console.log(`DMG ${observed.dmgAsset}`);
	console.log(`SHA-256 ${staged.sha256}`);
	finishRelease(repo, observed, decision.createTag, staged.dmg, staged.checksum, notes);
}

function main(): void {
	if (process.argv.includes('--help') || process.argv.includes('-h')) {
		console.log(HELP);
		return;
	}
	const [command, dir, ...extra] = process.argv.slice(2);
	if (extra.length > 0 || (command !== 'plan' && command !== 'create')) {
		throw new Error(HELP);
	}
	if (command === 'create' && !dir) throw new Error(HELP);
	const repoRoot = path.resolve(import.meta.dirname, '..');
	if (command === 'plan') plan(repoRoot);
	else publish(repoRoot, path.resolve(dir ?? ''));
}

if (import.meta.main) {
	try {
		main();
	} catch (error) {
		console.error(`Error: ${error instanceof Error ? error.message : String(error)}`);
		process.exit(1);
	}
}
