export interface Timeline {
    timelineDay: number;
    timelineDate: string;
    timelinePlaying: boolean;
    timelineAnimationId: number | null;
    playbackSpeed: number;
    playbackMode: boolean;
}

export const timeline: Timeline = $state({
    timelineDay: 0,
    timelineDate: "",
    timelinePlaying: false,
    timelineAnimationId: null,
    playbackSpeed: 40,
    playbackMode: false,
});
