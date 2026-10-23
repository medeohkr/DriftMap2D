type NumberInputParams = {
    min?: number;
    max?: number;
    defaultValue?: number;
};

export function numberInput(
    node: HTMLInputElement,
    params: NumberInputParams = {}
) {
    let { min = -Infinity, max = Infinity, defaultValue = 0 } = params;
    
    // Initialize from current value or default
    let lastValid = parseFloat(node.value);
    if (isNaN(lastValid)) {
        lastValid = defaultValue;
    }
    
    function commit() {
        const parsed = parseFloat(node.value);
        
        let next: number;
        if (isNaN(parsed)) {
            // Restore last valid value
            next = lastValid;
        } else {
            // Clamp to min/max
            next = Math.max(min, Math.min(max, parsed));
        }
        
        // Only dispatch if the value actually changed
        if (next !== parsed) {
            node.value = String(next);
            lastValid = next;
            node.dispatchEvent(new Event("input", { bubbles: true }));
        } else {
            lastValid = next;
        }
    }
    
    function handleKeydown(e: KeyboardEvent) {
        // Block minus if min >= 0
        if (min >= 0 && e.key === "-") {
            e.preventDefault();
        }
        // Block 'e' (scientific notation) if min >= 0
        if (min >= 0 && e.key === "e") {
            e.preventDefault();
        }
    }
    
    node.addEventListener("blur", commit);
    node.addEventListener("keydown", handleKeydown);
    
    return {
        update(newParams: NumberInputParams) {
            min = newParams.min ?? -Infinity;
            max = newParams.max ?? Infinity;
            defaultValue = newParams.defaultValue ?? 0;
        },
        destroy() {
            node.removeEventListener("blur", commit);
            node.removeEventListener("keydown", handleKeydown);
        }
    };
}
