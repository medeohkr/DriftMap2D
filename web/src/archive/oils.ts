import arabianLight from '../../assets/oils/Arabian Light [2002].json';
import IFO380 from '../../assets/oils/IFO-380LS.json'
import marineDiesel from '../../assets/oils/Marine Diesel [2018].json'
import { dom } from './ui'

const oilMapping: Record<string, string> = {
    "arabian-light": wrapOilData(arabianLight),
    "bonny-light": wrapOilData(arabianLight),
    "marine-diesel": wrapOilData(marineDiesel),
    "venezuelan-heavy": wrapOilData(arabianLight),
    "ifo-380": wrapOilData(IFO380),
};

function wrapOilData(adiosJson: any): string {
    return JSON.stringify({
        type: "oil",
        adios_json: typeof adiosJson === 'string' ? adiosJson : JSON.stringify(adiosJson),
        overrides: {},
    });
}

export function getOilData(): string {
    const oilName = dom.basicOilMenu?.value || "arabian-light";
    const data = oilMapping[oilName];
    if (!data) {
        console.warn(`Oil "${oilName}" not found, using Arabian Light`);
        return oilMapping["arabian-light"] || "{}";
    }
    return data;
}



