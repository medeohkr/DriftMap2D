
// === DOM ELEMENTS ===
//tabs and panels
const basicTab = document.getElementById("tab-basic");
const advancedTab = document.getElementById("tab-advanced");
const basicPanel = document.getElementById("panel-basic");
const advancedPanel = document.getElementById("panel-advanced")

//model and tracer selectors
const modelMenu = document.getElementById("model-selector");
const oilMenu = document.getElementById("oil-selector");
const objectMenu = document.getElementById("object-selector");
const plasticMenu = document.getElementById("plastic-selector");

const modelMenuContainer = document.getElementById("model-selector-container");
const oilMenuContainer = document.getElementById("oil-selector-container");
const objectMenuContainer = document.getElementById("object-selector-container");
const plasticMenuContainer = document.getElementById("plastic-selector-container");

const basicModelMenu = document.getElementById("model-selector-basic");
const basicOilMenu = document.getElementById("oil-selector-basic");
const basicObjectMenu = document.getElementById("object-selector-basic");
const basicPlasticMenu = document.getElementById("plastic-selector-basic");

const basicModelMenuContainer = document.getElementById("model-selector-container-basic");
const basicOilMenuContainer = document.getElementById("oil-selector-container-basic");
const basicObjectMenuContainer = document.getElementById("object-selector-container-basic");
const basicPlasticMenuContainer = document.getElementById("plastic-selector-container-basic");

// === EVENT LISTENERS ===
//tabs
basicTab.addEventListener("click", () => setActiveTab(basicTab));
advancedTab.addEventListener("click", () => setActiveTab(advancedTab));

//model selector
modelMenu.addEventListener("change", updateTracerMenu);
basicModelMenu.addEventListener("change", updateBasicTracerMenu);

// === UI CHANGE FUNCTIONS ===
function setActiveTab(tab) {
    const isBasic = tab === basicTab;
    
    basicTab.classList.toggle("active", isBasic);
    advancedTab.classList.toggle("active", !isBasic);
    
    basicPanel.hidden = !isBasic;
    advancedPanel.hidden = isBasic;
}

function updateTracerMenu() {
    oilMenuContainer.classList.add("hidden");
    objectMenuContainer.classList.add("hidden");
    plasticMenuContainer.classList.add("hidden");
    
    if (modelMenu.value === "oil-weathering") {
        oilMenuContainer.classList.remove("hidden");
    } else if (modelMenu.value === "search-and-rescue") {
        objectMenuContainer.classList.remove("hidden");
    } else if (modelMenu.value === "plastic-drift") {
        plasticMenuContainer.classList.remove("hidden");
    }
}

function updateBasicTracerMenu() {
    basicOilMenuContainer.classList.add("hidden");
    basicObjectMenuContainer.classList.add("hidden");
    basicPlasticMenuContainer.classList.add("hidden");
    
    if (basicModelMenu.value === "oil-weathering") {
        basicOilMenuContainer.classList.remove("hidden");
    } else if (basicModelMenu.value === "search-and-rescue") {
        basicObjectMenuContainer.classList.remove("hidden");
    } else if (basicModelMenu.value === "plastic-drift") {
        basicPlasticMenuContainer.classList.remove("hidden");
    }
}
// === INITIALIZATION ===
updateTracerMenu();
updateBasicTracerMenu();