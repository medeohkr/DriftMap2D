<script lang="ts">
    import { sidebarState, simulation } from '$lib/stores/index.svelte';
    import UtilityBar from './UtilityBar.svelte';
    import ModelSelection from './config/ModelSelection.svelte';
    import SimulationTime from './config/SimulationTime.svelte';
    import ReleaseSettings from './config/ReleaseSettings.svelte';
    import ActionBar from './ActionBar.svelte';
    import TitleHeader from './TitleHeader.svelte';
    import AdvancedConfig from './config/AdvancedConfig.svelte';
    import TracerConfig from './config/TracerConfig.svelte';
    import VisualizationConfig from './config/VisualizationConfig.svelte';

    let collapseTimeout: ReturnType<typeof setTimeout>;
    let transitionTimeout: ReturnType<typeof setTimeout>;
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
        }, 200);
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

    <fieldset disabled={simulation.simulationActive}>
        <div class="sidebar-content">
                <div class="model-selection">
                    <ModelSelection />
                    <details class="floating-container">
                        <TracerConfig />
                    </details>
                </div>

                <SimulationTime />

                <ReleaseSettings />

                <details class="floating-container">
                    <AdvancedConfig />
                </details>

                <details class="floating-container">
                    <VisualizationConfig />
                </details>
        </div>
    </fieldset>
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
    padding: 14px;
    row-gap: var(--spacing-lg);
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

fieldset {
    border: none;
    margin: 0;
    padding: 0;
    min-width: 0;
    
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0; 
    
    transition: all var(--transition-medium);
}

fieldset[disabled] {
    opacity: 0.6;
}
</style>
