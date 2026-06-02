<script lang="ts">
	import { openRemoteSourceAcquire } from './state.svelte';
</script>

<button
	id="acquire-audiobooks-btn"
	class="btn-pill btn-pill-secondary"
	type="button"
	onclick={openRemoteSourceAcquire}
>
	Acquire
</button>
