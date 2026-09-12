export interface Release {
    lat: number;
    lon: number;
    radius: number;
    schedule: Schedule[];
}

export interface Schedule {
    amount: number;
    duration: number;
}

export const releaseConfig = $state({
    releases: [
        {
            lat: 26.58,
            lon: 56.25,
            radius: 2.5,
            schedule: [
                {
                    amount: 100,
                    duration: 12,
                },
            ],
        },
    ],
    activeReleaseIndex: 0,

    get activeRelease() {
        return this.releases[this.activeReleaseIndex];
    },

    addRelease(release: any) {
        releaseConfig.releases.push({
            lat: releaseConfig.activeRelease.lat + 0.02,
            lon: releaseConfig.activeRelease.lon + 0.02,
            radius: releaseConfig.activeRelease.radius,
            schedule: [
                {
                    amount: 100,
                    duration: 12,
                },
            ],
            ...release,
        });
        releaseConfig.activeReleaseIndex = releaseConfig.releases.length - 1;
    },

    removeRelease(index: number) {
        if (releaseConfig.releases.length <= 1) return;
        if (index === releaseConfig.activeReleaseIndex) {
            releaseConfig.activeReleaseIndex = Math.max(0, index - 1);
        } else if (index < releaseConfig.activeReleaseIndex) {
            releaseConfig.activeReleaseIndex--;
        }
        releaseConfig.releases.splice(index, 1);
    },

    addInterval(interval: any) {
        releaseConfig.activeRelease.schedule.push({
            amount: 100,
            duration: 12,
            ...interval,
        });
    },

    removeInterval(index: number) {
        releaseConfig.activeRelease.schedule.splice(index, 1);
    },
});