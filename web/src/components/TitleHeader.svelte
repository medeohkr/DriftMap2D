<script lang="ts">
    import logo from "$lib/assets/images/DriftMap2D.webp";
    import importSymbol from "$lib/assets/images/Import.webp"
    import resetSymbol from "$lib/assets/images/ResetConfig.webp"
</script>

<div class="title-container">
    <img class="logo" src={logo} alt="DriftMap Logo" />
    <div class="title-version-container">
        <span class="title-text">DriftMap2D</span>
        <span class="version-text">v0.1</span>
    </div>
</div>

<style>
.title-container {
    display: flex;
    width: var(--width-sidebar);
    height: var(--height-title-header);
    padding: var(--spacing-sm) var(--spacing-md);

    flex-shrink: 0;
    column-gap: var(--spacing-sm);
}

.logo {
    height: 100%;
    opacity: 0.6;
}

.title-version-container {
    display: flex;
    flex-direction: column;
}

.title-text {
    display: inline-block;
    font-family: var(--font-family);
    font-size: var(--font-size-lg);
    font-weight: var(--weight-primary);
    color: var(--text-primary);
}

.version-text {
    font-family: var(--font-family);
    font-size: var(--font-size-xs);
    font-weight: var(--weight-primary);
    color: var(--text-muted);
}

</style>
