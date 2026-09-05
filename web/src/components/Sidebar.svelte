<script lang="ts">
    import { sidebarState } from '$lib/stores.svelte';
    import UtilityBar from './UtilityBar.svelte';
    import ModelSelection from './config/ModelSelection.svelte';
    import SimulationTime from './config/SimulationTime.svelte';
    import ReleaseSettings from './config/ReleaseSettings.svelte';
    import ActionBar from './ActionBar.svelte';
    import TitleHeader from './TitleHeader.svelte';

    let collapseTimeout = null;
    let transitionTimeout = null;
    let isTransitioning = $state(false);
    
    function toggleSidebar() {
        isTransitioning = true;
        
        if (sidebarState.collapseStage === 0) {
            sidebarState.collapseStage = 1;
            collapseTimeout = setTimeout(() => {
                sidebarState.collapseStage = 2;
            }, 300);
        } else {
            clearTimeout(collapseTimeout);
            sidebarState.collapseStage = 0;
        }
        
        clearTimeout(transitionTimeout);
        transitionTimeout = setTimeout(() => {
            isTransitioning = false;
        }, 150);
    }

</script>

<div 
    class="sidebar"
    class:stage-1={sidebarState.collapseStage === 1}
    class:stage-2={sidebarState.collapseStage === 2}
    class:transitioning={isTransitioning}
>
    <div class="title-header">
        <TitleHeader />
    </div>

    <div class="action-bar">
        <ActionBar {toggleSidebar} />
    </div>

    <div class="utility-bar">
        <UtilityBar />
    </div>
    <div class="sidebar-content">
        <div class="model-selection">
            <ModelSelection />
        </div>

        <SimulationTime />
        <ReleaseSettings />
    </div>
</div>


<style>
.title-header {
    height: var(--height-title-header);
    overflow: hidden;
    transition: 
        height var(--transition-slow),
        opacity var(--transition-fast),
        transform var(--transition-medium);
    transform-origin: top;
    opacity: 1;
}

.action-bar {
    display: flex;
    width: 100%;
    padding: 0 var(--spacing-sm);
    background-color: var(--bg-secondary);
    flex-shrink: 0;
    align-items: center;
    column-gap: var(--spacing-md);
}

.utility-bar {
    height: var(--height-utility-bar);
    overflow: hidden;
    transition: all var(--transition-medium);
    opacity: 1;
}

.sidebar {
    position: absolute;
    top: var(--spacing-sm);
    bottom: var(--spacing-sm);
    left: var(--spacing-sm);
    display: flex;
    overflow: hidden;
    flex-direction: column;
    width: var(--width-sidebar);
    max-height: calc(100vh - 2 * var(--spacing-sm));
    background: var(--bg-primary);
    box-shadow: 2px 2px 8px var(--shadow-secondary);
    transition: max-height var(--transition-slow);
}

.sidebar.stage-1 {
    max-height: calc(60px + var(--height-action-btn) + var(--height-utility-bar));
}

.sidebar.stage-2 {
    max-height: var(--height-action-btn);
}

/* Collapsed states */
.sidebar.stage-2 .title-header {
    height: 0;
    opacity: 0;
    transform: translateY(-5px) scaleY(0.9);
    pointer-events: none;
}

.sidebar.stage-2 .utility-bar {
    height: 0;
    opacity: 0;
    pointer-events: none;
}

.sidebar.stage-1 .sidebar-content,
.sidebar.stage-2 .sidebar-content {
    flex: 0;
    height: 0;
    min-height: 0;
    padding: 0;
    opacity: 0;
    pointer-events: none;
}

.sidebar-content {
    display: flex;
    overflow-x: hidden;
    overflow-y: auto;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    padding: var(--spacing-md);
    row-gap: var(--spacing-md);
    transition: all var(--transition-medium);
    opacity: 1;
}
.sidebar.transitioning .sidebar-content {
    overflow: hidden;
}

.model-selection {
    display: flex;
    flex-direction: column;
    row-gap: var(--spacing-sm);
}
</style>
