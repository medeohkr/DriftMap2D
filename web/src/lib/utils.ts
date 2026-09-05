import { config } from "./stores.svelte";

export function dateOffset(days: number) {
    const date = new Date();
    date.setDate(date.getDate() + days);
    return date.toISOString().split("T")[0];
}

export function getTotalDays(): number {
    const startDateTime = `${config.startDate}T${config.startTime}`
    const endDateTime = `${config.endDate}T${config.endTime}`
    const start = new Date(startDateTime).getTime();
    const end = new Date(endDateTime).getTime();
    return (end - start) / (1000 * 60 * 60 * 24);
}

export function startDateTime() {
    return `${config.startDate} ${config.startTime}`
}