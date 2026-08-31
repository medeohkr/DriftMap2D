<script lang="ts">
    import logo from "$lib/assets/images/DriftMap.webp";
    import BasicPanel from "./basic/BasicPanel.svelte";
    import AdvancedPanel from "./advanced/AdvancedPanel.svelte";
    let activeTab: "basic" | "advanced" = "basic";
</script>

<div class="sidebar-container">
    <div class="sidebar" id="sidebar">
        <!-- header -->
        <div class="header-container">
            <img class="logo" src={logo} alt="DriftMap Logo" />
            <span class="title-text">DriftMap</span>
        </div>
        <!-- tabs -->
        <div class="sidebar-body">
            <div class="tabs-container">
                <button
                    class="tab"
                    id="tab-basic"
                    class:active={activeTab === "basic"}
                    on:click={() => (activeTab = "basic")}>BASIC</button
                >
                <button
                    class="tab"
                    id="tab-advanced"
                    class:active={activeTab === "advanced"}
                    on:click={() => (activeTab = "advanced")}>ADVANCED</button
                >
            </div>
            <!-- basic panel -->
            {#if activeTab === "basic"}
                <BasicPanel />
            {:else}
                <AdvancedPanel />
            {/if}
        </div>
    </div>
    <div class="sidebar-border"></div>
</div>

<style>
    .sidebar-container {
        position: absolute;
        top: 0;
        left: 0;
        height: 100vh;
        display: flex;
        box-shadow: var(--shadow-size-sidebar) var(--shadow-primary);
    }

    .sidebar {
        display: flex;
        flex-direction: column;
        height: 100vh;
        background: var(--bg-primary);
    }

    .sidebar-border {
        width: var(--width-border);
        height: 100vh;
        background-color: var(--bg-secondary);
    }

    .sidebar-body {
        height: 100%;
        flex: 1;
        overflow-y: auto;
        display: flex;
        flex-direction: column;
        align-items: center;
        scrollbar-width: thin;
        scrollbar-color: var(--scrollbar);
    }

    .header-container {
        display: flex;
        flex-shrink: 0;
        height: 80px;
        align-items: center;
        justify-content: center;
        background-color: var(--bg-header);
        width: var(--width-sidebar);
    }

    .logo {
        width: var(--width-logo);
        padding: var(--spacing-sm);
    }

    .title-text {
        color: var(--text-primary);
        font-family: var(--font-family);
        font-size: var(--font-size-title);
        font-weight: var(--weight-primary);
        padding: var(--spacing-sm);
    }

    .tabs-container {
        width: 100%;
        display: flex;
    }

    .tab {
        all: unset;
        flex: 1;
        text-align: center;
        padding: var(--spacing-xs);
        background-color: var(--bg-primary);
        color: var(--text-muted);
        font-family: var(--font-family);
        font-weight: var(--weight-primary);
        box-shadow: var(--shadow-size-secondary) var(--shadow-secondary);
        cursor: pointer;
    }

    .tab.active {
        background-color: var(--bg-secondary);
        cursor: default;
    }
</style>
