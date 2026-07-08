//! Scenario packaging primitives for downstream ASP Rust harness users.

/// One command expectation attached to a custom ASP Rust harness scenario.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AspRustProjectHarnessScenarioCommand {
    pub label: &'static str,
    pub argv: &'static [&'static str],
}

/// A reusable custom scenario owned by the ASP Rust harness policy crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AspRustProjectHarnessScenario {
    pub name: &'static str,
    pub package_name: &'static str,
    pub description: &'static str,
    pub fixture_root: &'static str,
    pub tags: &'static [&'static str],
    pub commands: &'static [AspRustProjectHarnessScenarioCommand],
}

/// A package of custom scenarios that a member crate can expose from tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspRustProjectHarnessScenarioPackage {
    pub package_name: &'static str,
    pub scenarios: Vec<AspRustProjectHarnessScenario>,
}

/// Builds an ASP Rust harness scenario from declarative package data.
#[macro_export]
macro_rules! asp_rust_project_harness_scenario {
    (
        name: $name:expr,
        package: $package_name:expr,
        description: $description:expr,
        fixture_root: $fixture_root:expr,
        tags: [$($tag:expr),* $(,)?],
        commands: [
            $(
                {
                    label: $label:expr,
                    argv: [$($argv:expr),* $(,)?]
                }
            ),* $(,)?
        ] $(,)?
    ) => {
        $crate::AspRustProjectHarnessScenario {
            name: $name,
            package_name: $package_name,
            description: $description,
            fixture_root: $fixture_root,
            tags: &[$($tag),*],
            commands: &[
                $(
                    $crate::AspRustProjectHarnessScenarioCommand {
                        label: $label,
                        argv: &[$($argv),*],
                    }
                ),*
            ],
        }
    };
}

/// Builds a package-level collection of ASP Rust harness scenarios.
#[macro_export]
macro_rules! asp_rust_project_harness_scenario_package {
    (
        package: $package_name:expr,
        scenarios: [$($scenario:expr),* $(,)?] $(,)?
    ) => {
        $crate::AspRustProjectHarnessScenarioPackage {
            package_name: $package_name,
            scenarios: vec![$($scenario),*],
        }
    };
}
