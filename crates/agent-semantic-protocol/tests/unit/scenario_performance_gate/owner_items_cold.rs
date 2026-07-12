use super::owner_items;

pub(crate) fn asp_rust_owner_items_cold_functional_path_stays_inside_scenario_gate() {
    assert_owner_items_cold_functional_path(OwnerItemsColdFunctionalScenario {
        scenario_dir: "asp_rust_owner_items_cold_functional_path",
        scenario_id: "asp-rust-owner-items-cold-functional-path",
        language_id: "rust",
        binary: "rs-harness",
        owner_path: "crate/src/lib.rs",
        package_anchor_path: "Cargo.toml",
        package_anchor_text: "[package]\nname = \"scenario-rust-owner-items-cold-functional\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        source_text: "pub async fn dynamic_owner_item_index() {}\n",
        query: "dynamic_owner_item_index",
        alg: "rust-harness-owner-items",
        item_symbol: "dynamic_owner_item_index",
    });
}

pub(crate) fn asp_typescript_owner_items_cold_functional_path_stays_inside_scenario_gate() {
    assert_owner_items_cold_functional_path(OwnerItemsColdFunctionalScenario {
        scenario_dir: "asp_typescript_owner_items_cold_functional_path",
        scenario_id: "asp-typescript-owner-items-cold-functional-path",
        language_id: "typescript",
        binary: "ts-harness",
        owner_path: "app/src/model.ts",
        package_anchor_path: "package.json",
        package_anchor_text: "{\"name\":\"scenario-typescript-owner-items-cold-functional\",\"private\":true}\n",
        source_text: "export function dynamicOwnerItemIndex(): boolean { return true; }\n",
        query: "dynamicOwnerItemIndex",
        alg: "ts-harness-owner-items",
        item_symbol: "dynamicOwnerItemIndex",
    });
}

pub(crate) fn asp_python_owner_items_cold_functional_path_stays_inside_scenario_gate() {
    assert_owner_items_cold_functional_path(OwnerItemsColdFunctionalScenario {
        scenario_dir: "asp_python_owner_items_cold_functional_path",
        scenario_id: "asp-python-owner-items-cold-functional-path",
        language_id: "python",
        binary: "py-harness",
        owner_path: "src/model.py",
        package_anchor_path: "pyproject.toml",
        package_anchor_text: "[project]\nname = \"scenario-python-owner-items-cold-functional\"\nversion = \"0.1.0\"\n",
        source_text: "def dynamic_owner_item_index() -> bool:\n    return True\n",
        query: "dynamic_owner_item_index",
        alg: "py-harness-owner-items",
        item_symbol: "dynamic_owner_item_index",
    });
}

pub(crate) fn asp_julia_owner_items_cold_functional_path_stays_inside_scenario_gate() {
    assert_owner_items_cold_functional_path(OwnerItemsColdFunctionalScenario {
        scenario_dir: "asp_julia_owner_items_cold_functional_path",
        scenario_id: "asp-julia-owner-items-cold-functional-path",
        language_id: "julia",
        binary: "asp-julia-harness",
        owner_path: "src/Model.jl",
        package_anchor_path: "Project.toml",
        package_anchor_text: "name = \"ScenarioJuliaOwnerItemsColdFunctional\"\nversion = \"0.1.0\"\n",
        source_text: "dynamic_owner_item_index() = true\n",
        query: "dynamic_owner_item_index",
        alg: "asp-julia-harness-owner-items",
        item_symbol: "dynamic_owner_item_index",
    });
}

pub(super) struct OwnerItemsColdFunctionalScenario {
    pub(crate) scenario_dir: &'static str,
    pub(crate) scenario_id: &'static str,
    pub(crate) language_id: &'static str,
    pub(crate) binary: &'static str,
    pub(crate) owner_path: &'static str,
    pub(crate) package_anchor_path: &'static str,
    pub(crate) package_anchor_text: &'static str,
    pub(crate) source_text: &'static str,
    pub(crate) query: &'static str,
    pub(crate) alg: &'static str,
    pub(crate) item_symbol: &'static str,
}

pub(super) fn assert_owner_items_cold_functional_path(spec: OwnerItemsColdFunctionalScenario) {
    owner_items::assert_owner_items_cold_functional_path(spec);
}
