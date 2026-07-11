<script lang="ts">
	import FileInspectorPanel from './FileInspectorPanel.svelte';
	import InputWorkflowPanel from './InputWorkflowPanel.svelte';
	import type { ReadWorkActivityByInputId } from '../fileList';

	interface Props {
		readWorkActivityByInputId?: ReadWorkActivityByInputId;
	}

	let { readWorkActivityByInputId }: Props = $props();
</script>

<div class="panel input-panel left-column-wrapper" data-testid="left-column">
	<InputWorkflowPanel {readWorkActivityByInputId} />
	<FileInspectorPanel />
</div>
