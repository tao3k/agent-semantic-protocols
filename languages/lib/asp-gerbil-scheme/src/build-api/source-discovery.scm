(declare (block) (standard-bindings) (extended-bindings))
(begin
  (begin
    (load-module "clan/building")
    (load-module "clan/filesystem")
    (load-module "std/misc/ports")
    (load-module "std/misc/process")
    (load-module "std/sort")
    (load-module "std/srfi/1")
    (load-module "std/misc/list")
    (load-module "std/srfi/13")
    (load-module "std/misc/string"))
  (load-module "asp-gerbil-scheme/src/build-api/source-discovery~0"))
