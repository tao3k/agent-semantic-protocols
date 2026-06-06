from @file file, string name, string prefix
where files(file, name) and sourceLocationPrefix(prefix) and name = prefix + "/src/lib.rs"
select "src/lib.rs"
