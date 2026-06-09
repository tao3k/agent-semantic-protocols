//! Search-pipe surface parsing and normalization.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchSurface {
    Owner,
    Items,
    Tests,
    Deps,
}

impl SearchSurface {
    fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Items => "items",
            Self::Tests => "tests",
            Self::Deps => "deps",
        }
    }
}

pub(super) fn default_search_surfaces() -> Vec<String> {
    [
        SearchSurface::Owner,
        SearchSurface::Items,
        SearchSurface::Tests,
    ]
    .into_iter()
    .map(SearchSurface::as_str)
    .map(ToOwned::to_owned)
    .collect()
}

pub(super) fn parse_search_surfaces(value: &str) -> Result<Vec<String>, String> {
    let surfaces = value
        .split(',')
        .map(str::trim)
        .filter(|surface| !surface.is_empty())
        .map(parse_search_surface)
        .try_fold(Vec::new(), |surfaces, surface| {
            surface.map(|surface| push_unique_surface(surfaces, surface))
        })?;
    if surfaces.is_empty() {
        return Err("--surface requires at least one surface".to_string());
    }
    Ok(surfaces)
}

pub(super) fn normalized_search_surfaces(surfaces: &[String]) -> Vec<String> {
    let normalized = surfaces
        .iter()
        .filter_map(|surface| parse_search_surface(surface).ok())
        .fold(Vec::new(), push_unique_surface);
    if normalized.is_empty() {
        default_search_surfaces()
    } else {
        normalized
    }
}

pub(super) fn include_owner_context(surfaces: &[String]) -> bool {
    include_owner(surfaces)
        || include_items(surfaces)
        || include_tests(surfaces)
        || include_deps(surfaces)
}

pub(super) fn include_items(surfaces: &[String]) -> bool {
    surfaces.iter().any(|surface| surface == "items")
}

pub(super) fn include_tests(surfaces: &[String]) -> bool {
    surfaces.iter().any(|surface| surface == "tests")
}

pub(super) fn include_deps(surfaces: &[String]) -> bool {
    surfaces.iter().any(|surface| surface == "deps")
}

fn include_owner(surfaces: &[String]) -> bool {
    surfaces.iter().any(|surface| surface == "owner")
}

fn parse_search_surface(surface: &str) -> Result<SearchSurface, String> {
    match surface {
        "owner" => Ok(SearchSurface::Owner),
        "item" | "items" => Ok(SearchSurface::Items),
        "test" | "tests" => Ok(SearchSurface::Tests),
        "dep" | "deps" | "dependency" | "dependencies" => Ok(SearchSurface::Deps),
        _ => Err(format!(
            "unknown search surface: {surface} (expected owner,items,tests,deps)"
        )),
    }
}

fn push_unique_surface(mut surfaces: Vec<String>, surface: SearchSurface) -> Vec<String> {
    let surface = surface.as_str();
    if !surfaces.iter().any(|item| item == surface) {
        surfaces.push(surface.to_string());
    }
    surfaces
}
