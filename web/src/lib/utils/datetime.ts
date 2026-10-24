import { config } from "../stores/index.svelte";

export function dateOffset(days: number) {
    const date = new Date();
    date.setUTCDate(date.getUTCDate() + days);
    return date.toISOString().split("T")[0];
}

export function getTotalDays(): number {
    const startDateTime = `${config.startDate}T${config.startTime}`
    const endDateTime = `${config.endDate}T${config.endTime}`
    const start = new Date(`${startDateTime}Z`).getTime();
    const end = new Date(`${endDateTime}Z`).getTime();
    return (end - start) / (1000 * 60 * 60 * 24);
}

export function startDateTime() {
    return `${config.startDate} ${config.startTime}`
}
