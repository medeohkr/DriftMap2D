export interface Toast {
    message: string;
    visible: boolean;
    timeoutId: ReturnType<typeof setTimeout> | null;
}

export const toast: Toast = $state({
    message: "",
    visible: false,
    timeoutId: null,
});

export function showToast(message: string, duration: number = 3500) {
    if (toast.timeoutId) {
        clearTimeout(toast.timeoutId);
    }
    
    toast.message = message;
    toast.visible = true;
    console.log(toast.visible)
    toast.timeoutId = setTimeout(() => {
        toast.visible = false;
        toast.timeoutId = null;
    }, duration);
}