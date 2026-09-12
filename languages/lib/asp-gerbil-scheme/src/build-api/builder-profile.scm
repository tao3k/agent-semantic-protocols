(declare (block) (standard-bindings) (extended-bindings))
(begin
  (begin
    (load-module "clan/poo/object")
    (load-module "asp-gerbil-scheme/src/object-family/syntax")
    (load-module "asp-gerbil-scheme/src/build-api/build-environment-profile")
    (load-module "asp-gerbil-scheme/src/build-api/source-discovery")
    (load-module "std/srfi/13"))
  (load-module "asp-gerbil-scheme/src/build-api/builder-profile~0"))
