
// === DOM elements ===
//tabs and panels
const basicTab = document.getElementById("tab-basic");
const advancedTab = document.getElementById("tab-advanced");
const basicPanel = document.getElementById("panel-basic");
const advancedPanel = document.getElementById("panel-advanced")

//model and tracer selectors
const modelSelector = document.getElementById("model-selector");
const oilSelector = document.getElementById("oil-selector");
const objectSelector = document.getElementById("object-selector");
const plasticSelector = document.getElementById("plastic-selector");

const modelSelectorContainer = document.getElementById("model-selector-container");
const oilSelectorContainer = document.getElementById("oil-selector-container");
const objectSelectorContainer = document.getElementById("object-selector-container");
const plasticSelectorContainer = document.getElementById("plastic-selector-container");

//temporary for development
basicTab.classList.remove("active");
advancedTab.classList.add("active");
basicPanel.hidden = true;
advancedPanel.hidden = false;

// === event listeners ===
//tabs
basicTab.addEventListener("click", () => setActiveTab(basicTab));
advancedTab.addEventListener("click", () => setActiveTab(advancedTab));

//model selector
modelSelector.addEventListener("change", updateTracerSelector);

// === UI change functions ===
function setActiveTab(tab) {
    const isBasic = tab === basicTab;
    
    basicTab.classList.toggle("active", isBasic);
    advancedTab.classList.toggle("active", !isBasic);
    
    basicPanel.hidden = !isBasic;
    advancedPanel.hidden = isBasic;
}

function updateTracerSelector() {
    oilSelectorContainer.classList.add("hidden");
    objectSelectorContainer.classList.add("hidden");
    plasticSelectorContainer.classList.add("hidden");
    
    if (modelSelector.value === "oil-weathering") {
        oilSelectorContainer.classList.remove("hidden");
    } else if (modelSelector.value === "search-and-rescue") {
        objectSelectorContainer.classList.remove("hidden");
    } else if (modelSelector.value === "plastic-drift") {
        plasticSelectorContainer.classList.remove("hidden");
    }
}

// === initialize ===
updateTracerSelector();