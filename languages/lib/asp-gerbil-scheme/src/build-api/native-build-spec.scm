(declare (block) (standard-bindings) (extended-bindings))
(begin
  (begin
    (load-module "std/srfi/13")
    (load-module "asp-gerbil-scheme/src/build-api/package-build")
    (load-module "asp-gerbil-scheme/src/build-api/source-coverage")
    (load-module "asp-gerbil-scheme/src/build-api/package-native-plan"))
  (load-module "asp-gerbil-scheme/src/build-api/native-build-spec~0"))
