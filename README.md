# Condé Nast terraform

A small wrapper around the `terraform` CLI, to make it easier to run individual modules locally, from within Condé Nast's canonical terraform directory structure.

A module's backend config and var file live in a dir structure like: `$INFRA_DIR/$ENVIRONMENT/$REGION/$MODULE_NAME/*.tfvars`<br>
This wrapper remembers the previously used values of the path segments and allows the user to interactively change one or more of them on `condeform init`, making it a little easier to `init` and switch between environments and regions for any given module.

Previously used values are cached on a per-repo basis.


### Usage

Navigate to your module and `init`:
```sh
cd ./infra/terraform/vpc
condeform init
condeform plan
terraform plan.plan
```

### Build

```sh
cargo build -r
```
