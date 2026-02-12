import { spawnSync } from 'node:child_process';

function runGit(args, cwd) {
	const proc = spawnSync('git', args, {
		cwd,
		encoding: 'utf8',
	});

	if (proc.status !== 0) {
		return null;
	}

	return proc.stdout.trim();
}

export function getGitInfo(cwd) {
	const branch = runGit(['rev-parse', '--abbrev-ref', 'HEAD'], cwd);
	const commit = runGit(['rev-parse', '--short', 'HEAD'], cwd);
	const dirtyRaw = runGit(['status', '--porcelain'], cwd);

	return {
		branch: branch || 'unknown',
		commit: commit || 'unknown',
		dirty: Boolean(dirtyRaw),
	};
}
