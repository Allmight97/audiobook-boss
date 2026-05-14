import path from 'node:path';

import {
	assertSupportedMacOsHost,
	buildTauriApp,
	installLocalApplicationBundle,
	pruneLocalInstallArtifacts,
	resolveMacOsBundlePaths,
	verifyMacOsBundle,
} from './build-app';

function main(): void {
	if (process.platform !== 'darwin') {
		throw new Error('Local app installation is only supported on macOS.');
	}

	assertSupportedMacOsHost();

	const repoRoot = path.resolve(import.meta.dir, '..');
	const args = process.argv.slice(2);
	const skipBuild = args.includes('--skip-build');

	if (!skipBuild) {
		buildTauriApp(repoRoot, ['--bundles', 'app']);
	}

	const bundlePaths = resolveMacOsBundlePaths(repoRoot);
	verifyMacOsBundle(bundlePaths);
	installLocalApplicationBundle(bundlePaths);

	console.log(`Installed local app bundle: ${bundlePaths.applicationsAppPath}`);

	const removedPaths = pruneLocalInstallArtifacts(bundlePaths);
	if (removedPaths.length > 0) {
		console.log(`Removed local build install artifacts: ${removedPaths.join(', ')}`);
	}
}

if (import.meta.main) {
	main();
}
