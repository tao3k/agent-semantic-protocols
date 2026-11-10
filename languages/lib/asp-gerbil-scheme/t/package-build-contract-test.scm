(declare (block) (standard-bindings) (extended-bindings))
(begin
  (begin
    (load-module "gerbil/gambit")
    (load-module "std/test")
    (load-module "asp-gerbil-scheme/src/build-api/package-build")
    (load-module "asp-gerbil-scheme/src/build-api/native-build-spec")
    (load-module "asp-gerbil-scheme/src/build-api/package-native-plan"))
  (load-module "asp-gerbil-scheme/t/package-build-contract-test~0"))
