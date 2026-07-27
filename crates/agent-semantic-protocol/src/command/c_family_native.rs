use std::ffi::{CStr, CString, c_char, c_void};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeSourceRange {
    pub(crate) path: String,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) start_column: u32,
    pub(crate) end_column: u32,
    pub(crate) start_offset: u64,
    pub(crate) end_offset: u64,
    pub(crate) structural_selector: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeFact {
    pub(crate) name: String,
    pub(crate) qualified_name: String,
    pub(crate) symbol_id: String,
    pub(crate) semantic_variant_id: String,
    pub(crate) kind: String,
    pub(crate) role: String,
    pub(crate) visibility: String,
    pub(crate) r#type: String,
    pub(crate) target: String,
    pub(crate) target_symbol_id: String,
    pub(crate) container_symbol_id: String,
    pub(crate) translation_unit: String,
    pub(crate) compile_context_digest: String,
    pub(crate) location: NativeSourceRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeDependencyUsage {
    pub(crate) owner_path: String,
    pub(crate) translation_unit: String,
    pub(crate) compile_context_digest: String,
    pub(crate) semantic_variant_id: String,
    pub(crate) package_name: String,
    pub(crate) import_path: String,
    pub(crate) resolved_path: String,
    pub(crate) angled: bool,
    pub(crate) source_locator: String,
    pub(crate) query_keys: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeCompileContext {
    pub(crate) translation_unit: String,
    pub(crate) digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeParseResult {
    pub(crate) facts: Vec<NativeFact>,
    pub(crate) dependency_usages: Vec<NativeDependencyUsage>,
    pub(crate) compile_contexts: Vec<NativeCompileContext>,
    pub(crate) translation_units: Vec<String>,
    pub(crate) errors: Vec<String>,
}

#[repr(C)]
struct CSourceRangeV1 {
    path: *const c_char,
    start_line: u32,
    end_line: u32,
    start_column: u32,
    end_column: u32,
    start_offset: u64,
    end_offset: u64,
    structural_selector: *const c_char,
}

#[repr(C)]
struct CFactV1 {
    name: *const c_char,
    qualified_name: *const c_char,
    symbol_id: *const c_char,
    semantic_variant_id: *const c_char,
    kind: *const c_char,
    role: *const c_char,
    visibility: *const c_char,
    r#type: *const c_char,
    target: *const c_char,
    target_symbol_id: *const c_char,
    container_symbol_id: *const c_char,
    translation_unit: *const c_char,
    compile_context_digest: *const c_char,
    location: CSourceRangeV1,
}

#[repr(C)]
struct CDependencyUsageV1 {
    owner_path: *const c_char,
    translation_unit: *const c_char,
    compile_context_digest: *const c_char,
    semantic_variant_id: *const c_char,
    package_name: *const c_char,
    import_path: *const c_char,
    resolved_path: *const c_char,
    angled: u8,
    source_locator: *const c_char,
}

#[repr(C)]
struct CCompileContextV1 {
    translation_unit: *const c_char,
    digest: *const c_char,
}

enum CResultV1 {}

type AbiVersionFn = unsafe extern "C" fn() -> u32;
type ParseTranslationUnitFn = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    *const c_char,
    *const *const c_char,
    usize,
) -> *mut CResultV1;
type FreeResultFn = unsafe extern "C" fn(*mut CResultV1);
type CountFn = unsafe extern "C" fn(*const CResultV1) -> usize;
type FactAtFn = unsafe extern "C" fn(*const CResultV1, usize, *mut CFactV1) -> u8;
type DependencyAtFn = unsafe extern "C" fn(*const CResultV1, usize, *mut CDependencyUsageV1) -> u8;
type DependencyQueryKeyCountFn = unsafe extern "C" fn(*const CResultV1, usize) -> usize;
type DependencyQueryKeyAtFn = unsafe extern "C" fn(*const CResultV1, usize, usize) -> *const c_char;
type CompileContextAtFn =
    unsafe extern "C" fn(*const CResultV1, usize, *mut CCompileContextV1) -> u8;
type StringAtFn = unsafe extern "C" fn(*const CResultV1, usize) -> *const c_char;

struct NativeSymbols {
    parse_translation_unit: ParseTranslationUnitFn,
    free_result: FreeResultFn,
    fact_count: CountFn,
    fact_at: FactAtFn,
    dependency_count: CountFn,
    dependency_at: DependencyAtFn,
    dependency_query_key_count: DependencyQueryKeyCountFn,
    dependency_query_key_at: DependencyQueryKeyAtFn,
    compile_context_count: CountFn,
    compile_context_at: CompileContextAtFn,
    translation_unit_count: CountFn,
    translation_unit_at: StringAtFn,
    error_count: CountFn,
    error_at: StringAtFn,
}

#[cfg(unix)]
pub(crate) struct NativeParserLibrary {
    handle: *mut c_void,
    symbols: NativeSymbols,
}

#[cfg(unix)]
impl NativeParserLibrary {
    pub(crate) fn open(
        path: &Path,
        expected_abi_version: u32,
        parse_symbol: &str,
        free_symbol: &str,
    ) -> Result<Self, String> {
        let path = path
            .to_str()
            .ok_or_else(|| format!("native parser path is not UTF-8: {}", path.display()))?;
        let path =
            CString::new(path).map_err(|_| "native parser path contains a NUL byte".to_owned())?;
        let handle = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_LOCAL | libc::RTLD_NOW) };
        if handle.is_null() {
            return Err(last_dynamic_loader_error(
                "failed to open native parser library",
            ));
        }

        let loaded = (|| {
            let abi_version: AbiVersionFn = load_symbol(handle, "ccls_asp_abi_version")?;
            let actual_abi_version = unsafe { abi_version() };
            if actual_abi_version != expected_abi_version {
                return Err(format!(
                    "native parser ABI mismatch: expected {expected_abi_version}, got {actual_abi_version}"
                ));
            }

            Ok(NativeSymbols {
                parse_translation_unit: load_symbol(handle, parse_symbol)?,
                free_result: load_symbol(handle, free_symbol)?,
                fact_count: load_symbol(handle, "ccls_asp_result_fact_count_v1")?,
                fact_at: load_symbol(handle, "ccls_asp_result_fact_at_v1")?,
                dependency_count: load_symbol(handle, "ccls_asp_result_dependency_count_v1")?,
                dependency_at: load_symbol(handle, "ccls_asp_result_dependency_at_v1")?,
                dependency_query_key_count: load_symbol(
                    handle,
                    "ccls_asp_result_dependency_query_key_count_v1",
                )?,
                dependency_query_key_at: load_symbol(
                    handle,
                    "ccls_asp_result_dependency_query_key_at_v1",
                )?,
                compile_context_count: load_symbol(
                    handle,
                    "ccls_asp_result_compile_context_count_v1",
                )?,
                compile_context_at: load_symbol(handle, "ccls_asp_result_compile_context_at_v1")?,
                translation_unit_count: load_symbol(
                    handle,
                    "ccls_asp_result_translation_unit_count_v1",
                )?,
                translation_unit_at: load_symbol(handle, "ccls_asp_result_translation_unit_at_v1")?,
                error_count: load_symbol(handle, "ccls_asp_result_error_count_v1")?,
                error_at: load_symbol(handle, "ccls_asp_result_error_at_v1")?,
            })
        })();

        match loaded {
            Ok(symbols) => Ok(Self { handle, symbols }),
            Err(error) => {
                unsafe {
                    libc::dlclose(handle);
                }
                Err(error)
            }
        }
    }

    pub(crate) fn parse_translation_unit(
        &self,
        workspace: &Path,
        translation_unit: &str,
        language_id: &str,
        normalized_compile_args: &[String],
    ) -> Result<NativeParseResult, String> {
        let workspace = path_c_string(workspace, "workspace")?;
        let translation_unit = value_c_string(translation_unit, "translation unit")?;
        let language_id = value_c_string(language_id, "language id")?;
        let compile_args = normalized_compile_args
            .iter()
            .map(|argument| value_c_string(argument, "compiler argument"))
            .collect::<Result<Vec<_>, _>>()?;
        let compile_arg_pointers = compile_args
            .iter()
            .map(|argument| argument.as_ptr())
            .collect::<Vec<_>>();

        let raw = unsafe {
            (self.symbols.parse_translation_unit)(
                workspace.as_ptr(),
                translation_unit.as_ptr(),
                language_id.as_ptr(),
                compile_arg_pointers.as_ptr(),
                compile_arg_pointers.len(),
            )
        };
        if raw.is_null() {
            return Err("native parser returned a null result".to_owned());
        }
        let guard = NativeResultGuard {
            raw,
            free_result: self.symbols.free_result,
        };

        let facts = collect_facts(&self.symbols, guard.raw)?;
        let dependency_usages = collect_dependencies(&self.symbols, guard.raw)?;
        let compile_contexts = collect_compile_contexts(&self.symbols, guard.raw)?;
        let translation_units = collect_strings(
            guard.raw,
            self.symbols.translation_unit_count,
            self.symbols.translation_unit_at,
            "translation unit",
        )?;
        let errors = collect_strings(
            guard.raw,
            self.symbols.error_count,
            self.symbols.error_at,
            "parser error",
        )?;
        Ok(NativeParseResult {
            facts,
            dependency_usages,
            compile_contexts,
            translation_units,
            errors,
        })
    }
}

#[cfg(unix)]
impl Drop for NativeParserLibrary {
    fn drop(&mut self) {
        unsafe {
            libc::dlclose(self.handle);
        }
    }
}

struct NativeResultGuard {
    raw: *mut CResultV1,
    free_result: FreeResultFn,
}

impl Drop for NativeResultGuard {
    fn drop(&mut self) {
        unsafe {
            (self.free_result)(self.raw);
        }
    }
}

pub(crate) fn platform_library_filename(artifact_stem: &str) -> String {
    format!(
        "{}{}{}",
        std::env::consts::DLL_PREFIX,
        artifact_stem,
        std::env::consts::DLL_SUFFIX
    )
}

pub(crate) fn installed_library_path(activation_root: &Path, artifact_stem: &str) -> PathBuf {
    activation_root
        .join("lib")
        .join(platform_library_filename(artifact_stem))
}

#[cfg(unix)]
fn collect_facts(
    symbols: &NativeSymbols,
    result: *const CResultV1,
) -> Result<Vec<NativeFact>, String> {
    let count = unsafe { (symbols.fact_count)(result) };
    let mut facts = Vec::with_capacity(count);
    for index in 0..count {
        let mut fact = std::mem::MaybeUninit::<CFactV1>::uninit();
        if unsafe { (symbols.fact_at)(result, index, fact.as_mut_ptr()) } == 0 {
            return Err(format!("native parser omitted fact at index {index}"));
        }
        let fact = unsafe { fact.assume_init() };
        facts.push(NativeFact {
            name: copy_string(fact.name, "fact.name")?,
            qualified_name: copy_string(fact.qualified_name, "fact.qualified_name")?,
            symbol_id: copy_string(fact.symbol_id, "fact.symbol_id")?,
            semantic_variant_id: copy_string(fact.semantic_variant_id, "fact.semantic_variant_id")?,
            kind: copy_string(fact.kind, "fact.kind")?,
            role: copy_string(fact.role, "fact.role")?,
            visibility: copy_string(fact.visibility, "fact.visibility")?,
            r#type: copy_string(fact.r#type, "fact.type")?,
            target: copy_string(fact.target, "fact.target")?,
            target_symbol_id: copy_string(fact.target_symbol_id, "fact.target_symbol_id")?,
            container_symbol_id: copy_string(fact.container_symbol_id, "fact.container_symbol_id")?,
            translation_unit: copy_string(fact.translation_unit, "fact.translation_unit")?,
            compile_context_digest: copy_string(
                fact.compile_context_digest,
                "fact.compile_context_digest",
            )?,
            location: NativeSourceRange {
                path: copy_string(fact.location.path, "fact.location.path")?,
                start_line: fact.location.start_line,
                end_line: fact.location.end_line,
                start_column: fact.location.start_column,
                end_column: fact.location.end_column,
                start_offset: fact.location.start_offset,
                end_offset: fact.location.end_offset,
                structural_selector: copy_string(
                    fact.location.structural_selector,
                    "fact.location.structural_selector",
                )?,
            },
        });
    }
    Ok(facts)
}

#[cfg(unix)]
fn collect_dependencies(
    symbols: &NativeSymbols,
    result: *const CResultV1,
) -> Result<Vec<NativeDependencyUsage>, String> {
    let count = unsafe { (symbols.dependency_count)(result) };
    let mut dependencies = Vec::with_capacity(count);
    for index in 0..count {
        let mut dependency = std::mem::MaybeUninit::<CDependencyUsageV1>::uninit();
        if unsafe { (symbols.dependency_at)(result, index, dependency.as_mut_ptr()) } == 0 {
            return Err(format!("native parser omitted dependency at index {index}"));
        }
        let dependency = unsafe { dependency.assume_init() };
        let query_key_count = unsafe { (symbols.dependency_query_key_count)(result, index) };
        let mut query_keys = Vec::with_capacity(query_key_count);
        for query_key_index in 0..query_key_count {
            let value =
                unsafe { (symbols.dependency_query_key_at)(result, index, query_key_index) };
            query_keys.push(copy_string(value, "dependency.query_key")?);
        }
        dependencies.push(NativeDependencyUsage {
            owner_path: copy_string(dependency.owner_path, "dependency.owner_path")?,
            translation_unit: copy_string(
                dependency.translation_unit,
                "dependency.translation_unit",
            )?,
            compile_context_digest: copy_string(
                dependency.compile_context_digest,
                "dependency.compile_context_digest",
            )?,
            semantic_variant_id: copy_string(
                dependency.semantic_variant_id,
                "dependency.semantic_variant_id",
            )?,
            package_name: copy_string(dependency.package_name, "dependency.package_name")?,
            import_path: copy_string(dependency.import_path, "dependency.import_path")?,
            resolved_path: copy_string(dependency.resolved_path, "dependency.resolved_path")?,
            angled: dependency.angled != 0,
            source_locator: copy_string(dependency.source_locator, "dependency.source_locator")?,
            query_keys,
        });
    }
    Ok(dependencies)
}

#[cfg(unix)]
fn collect_compile_contexts(
    symbols: &NativeSymbols,
    result: *const CResultV1,
) -> Result<Vec<NativeCompileContext>, String> {
    let count = unsafe { (symbols.compile_context_count)(result) };
    let mut contexts = Vec::with_capacity(count);
    for index in 0..count {
        let mut context = std::mem::MaybeUninit::<CCompileContextV1>::uninit();
        if unsafe { (symbols.compile_context_at)(result, index, context.as_mut_ptr()) } == 0 {
            return Err(format!(
                "native parser omitted compile context at index {index}"
            ));
        }
        let context = unsafe { context.assume_init() };
        contexts.push(NativeCompileContext {
            translation_unit: copy_string(
                context.translation_unit,
                "compile_context.translation_unit",
            )?,
            digest: copy_string(context.digest, "compile_context.digest")?,
        });
    }
    Ok(contexts)
}

#[cfg(unix)]
fn collect_strings(
    result: *const CResultV1,
    count: CountFn,
    at: StringAtFn,
    label: &str,
) -> Result<Vec<String>, String> {
    let count = unsafe { count(result) };
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        values.push(copy_string(unsafe { at(result, index) }, label)?);
    }
    Ok(values)
}

#[cfg(unix)]
fn copy_string(value: *const c_char, label: &str) -> Result<String, String> {
    if value.is_null() {
        return Err(format!("native parser returned null for {label}"));
    }
    Ok(unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned())
}

#[cfg(unix)]
fn path_c_string(path: &Path, label: &str) -> Result<CString, String> {
    let value = path
        .to_str()
        .ok_or_else(|| format!("{label} is not UTF-8: {}", path.display()))?;
    value_c_string(value, label)
}

#[cfg(unix)]
fn value_c_string(value: &str, label: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("{label} contains a NUL byte"))
}

#[cfg(unix)]
fn load_symbol<T: Copy>(handle: *mut c_void, symbol: &str) -> Result<T, String> {
    let symbol = CString::new(symbol).map_err(|_| "symbol contains a NUL byte".to_owned())?;
    unsafe {
        libc::dlerror();
    }
    let address = unsafe { libc::dlsym(handle, symbol.as_ptr()) };
    if address.is_null() {
        return Err(last_dynamic_loader_error(
            "failed to load native parser symbol",
        ));
    }
    Ok(unsafe { std::mem::transmute_copy(&address) })
}

#[cfg(unix)]
fn last_dynamic_loader_error(prefix: &str) -> String {
    let error = unsafe { libc::dlerror() };
    if error.is_null() {
        return prefix.to_owned();
    }
    format!(
        "{prefix}: {}",
        unsafe { CStr::from_ptr(error) }.to_string_lossy()
    )
}
