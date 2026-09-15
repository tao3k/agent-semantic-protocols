# Input

Run the public V1 Scheme expression twice against the same ready Runtime:

```sh
asp search playbook '(search (producers (language gerbil-scheme) (documents org)) (intersect (rg "-n" "gerbil.pkg|gerbil-parser|meta-relational|MRR" ".") (tantivy "title:gerbil-parser^2 OR body:meta-relational OR body:gerbil.pkg")))'
```

The active provider registry contains `asp-gerbil-scheme` and contains no
`asp-org` provider artifact. Org files are discovered through the Orgize-owned
embedded document catalog.
