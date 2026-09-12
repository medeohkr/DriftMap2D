export interface SidebarState {
    collapseStage: number;
}

export const sidebarState: SidebarState = $state({
    collapseStage: 0,
});