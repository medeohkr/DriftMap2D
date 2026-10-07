<script lang="ts">
    import { toast } from '$lib/stores/toast.svelte';
</script>

{#if toast.visible}
    <div class="toast">
        {toast.message}
    </div>
{/if}

<style>
.toast {
    position: fixed;
    bottom: var(--spacing-lg);
    left: 50%;
    transform: translateX(-50%);
    padding: var(--spacing-sm) var(--spacing-md);
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: var(--border-sm) solid var(--border-sm);
    border-radius: var(--border-md);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    font-family: var(--font-family);
    font-size: var(--font-size-sm);
    animation: toast-in 0.2s ease;
    white-space: pre-line;
    line-height: 1.8;
}

@keyframes toast-in {
    from {
        opacity: 0;
        transform: translateX(-50%) translateY(10px);
    }
    to {
        opacity: 1;
        transform: translateX(-50%) translateY(0);
    }
}
</style>