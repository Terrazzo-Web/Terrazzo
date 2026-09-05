# Alpine Azure VM setup script plan

## Goal

Create `terminal/scripts/create-azure-vm.sh`. The script will take the VM name as
its only positional argument:

```sh
terminal/scripts/create-azure-vm.sh <vm-name>
```

It will create a minimal Alpine Linux VM and provision the Rust/Terrazzo tools.
Assume that `az login` has already been run. Do not create or change resources
outside `${vm}-rg`.

Required VM properties:

- resource group: `${vm}-rg`
- region: `germanywestcentral` (Germany West Central)
- size: `Standard_D2ps_v6` (2 vCPUs, 8 GiB)
- architecture: ARM64 (`Standard_D2ps_v6` is an Azure Cobalt ARM size)
- OS: official Alpine Linux ARM64 cloud image, using UEFI/Hyper-V generation 2
- Azure Linux Agent/WALA: absent, with Azure configured not to expect it
- admin/login user: `richard`
- authentication: SSH public key only; no user or root password login
- `richard` can use `sudo` without a password
- inbound TCP ports: 22, 80, and 443
- installed tools: Rust, the WASM Rust target, `wasm-pack`,
  `terrazzo-css-cli`, `protoc`, and `terrazzo-terminal --features prod`

The public key to install is:

```text
ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBaRBRd4kNHeFgqxi09MlCNmf6DT4ELjoDDWNXqJUuJp Richard @ MacBook Air
```

## Important design decisions

### Use Alpine's official downloadable Azure image

Alpine is not an Azure-endorsed Marketplace distribution. Third-party Alpine
Marketplace offers exist, but they are unnecessary and may add licensing,
image-plan, cost, or agent uncertainty. Alpine publishes Azure-ready VHDs
itself.

Pin the first implementation to this image rather than resolving "latest" at
runtime, so repeated runs use identical bytes:

```text
Alpine version: 3.24.1
VHD: azure_alpine-3.24.1-aarch64-uefi-cloudinit-r0.vhd
URL: https://dl-cdn.alpinelinux.org/alpine/v3.24/releases/cloud/azure_alpine-3.24.1-aarch64-uefi-cloudinit-r0.vhd
SHA-512: 89697f6ac1618845932dc8125fb774ee8401f97ad472e0043847518b95a738edaa580d8e91d13e8acebfb07feec799c07b48a95d8b61c3396da555eaaf3cdc27
```

This is Alpine's `aarch64`, UEFI, cloud-init, Azure image. It matches the ARM64
VM size and supports Azure provisioning through cloud-init without WALA. When
updating Alpine, update the URL, checksum, image metadata, and test the new
image together.

The official Alpine import guidance requires an Azure page blob, an Azure
Compute Gallery image definition marked ARM64, Hyper-V generation 2, Linux,
and generalized OS state. It also requires Security Type `Standard` when
launching the VM.

### Use cloud-init, not an Azure extension

Pass a `#cloud-config` file with `az vm create --custom-data`. Cloud-init is
already present in the selected Alpine image and supports Azure as a data
source. Set `--enable-agent false` on `az vm create`; do not install
`WALinuxAgent` or any Azure monitoring/management agent.

Consequences of having no WALA:

- Azure VM extensions are unavailable.
- `az vm run-command` and extension-based password/key recovery will not work.
- Provisioning, troubleshooting, and completion checks must use cloud-init,
  SSH, boot diagnostics, or the serial console.
- `az vm create` can return before cloud-init finishes the lengthy Cargo
  installs. A successful Azure deployment is not proof that tool installation
  succeeded.

### Use a Compute Gallery to retain ARM64 image metadata

Do not deploy the VHD as an untyped/specialized OS disk. The VM needs normal
generalized-image provisioning so cloud-init receives the hostname, requested
user, SSH key, and custom data. The gallery definition also explicitly records
`Arm64` and `V2`, avoiding an x64/default-generation mismatch.

## Ubuntu-to-Alpine command translation

The provisioning commands should use Alpine's package manager and native musl
toolchain:

| Ubuntu command/package | Alpine equivalent | Notes |
| --- | --- | --- |
| `apt update` | `apk update` | Cloud-init's `package_update: true` is preferred. |
| `apt dist-upgrade` | `apk upgrade` | Cloud-init's `package_upgrade: true` is preferred. |
| `build-essential` | `build-base` | Installs GCC, G++, make, libc headers, binutils, and related build tools. |
| `pkg-config` | `pkgconf` | Provides the `pkg-config` command. |
| `musl-tools` | none | Alpine is already musl-native; `build-base` supplies the development toolchain. |
| `libssl-dev` | `openssl-dev` | OpenSSL headers and pkg-config metadata. |
| `protobuf-compiler` | `protoc` | Provides the Protocol Buffer compiler. |
| `nano` | `nano` | Same package name. |
| `setcap` provider | `libcap-setcap` | Needed for binding ports 80/443 without running the server as root. |
| `sudo` | `sudo` | Needed for the requested passwordless sudo policy. |

Also install `curl` for rustup. Add `linux-headers` only if a real build failure
shows a crate requires kernel headers; it is not part of the known minimum.

On this ARM64 Alpine host, rustup selects
`aarch64-unknown-linux-musl` as the native toolchain. Do not add the Ubuntu
script's `x86_64-unknown-linux-musl` target: it is the wrong architecture and is
not needed to build the VM-local binary. Explicitly add only
`wasm32-unknown-unknown`; the native target is installed with the toolchain.

The requirement's “terrazzo-cli” refers to the crate used by this repository,
`terrazzo-css-cli`; there is no workspace crate named `terrazzo-cli`.

## Script implementation steps

### 1. Validate inputs and prerequisites

- Start with `#!/usr/bin/env bash` and `set -euo pipefail`.
- Require exactly one argument and assign it to `vm`; set
  `resource_group="${vm}-rg"`.
- Validate the VM name before using it in Azure resource names.
- Require `az`, `curl`, a SHA-512 checker (`sha512sum`, or `shasum -a 512` on
  macOS), and the utilities used for temporary files and name generation.
- Confirm login and an active subscription with `az account show`.
- Check that the resource group does not already exist. Refuse to mutate an
  existing `${vm}-rg`; this prevents a rerun from silently mixing resources or
  changing an existing VM.
- Query `az vm list-skus --location germanywestcentral --size
  Standard_D2ps_v6 --all` and fail with a useful message if the SKU is absent
  or restricted for the active subscription. Availability and quota are
  subscription-specific even though the region supports the size family.
- Use a cleanup trap only for local temporary files. Do not automatically
  delete the Azure resource group on failure, because that could hide useful
  diagnostics and is destructive.

### 2. Download and verify the Alpine VHD

- Create a local temporary directory with `mktemp -d`.
- Download the pinned VHD from Alpine over HTTPS.
- Verify it against the pinned SHA-512 above before uploading anything.
- Keep the download out of the repository and remove it in the local cleanup
  trap.

A server-side `az storage blob copy start` from the Alpine URL would avoid the
local download and is a possible later optimization, but it cannot directly
perform the pinned SHA-512 verification. Prefer verified bytes in the first
implementation.

### 3. Create the image resources in `${vm}-rg`

Run these operations in order:

1. `az group create --name "$resource_group" --location
   germanywestcentral`.
2. Create a `Standard_LRS`, `StorageV2` storage account in the same resource
   group. Storage account names are globally unique, lowercase alphanumeric,
   and at most 24 characters, so derive a deterministic short name from the
   subscription ID and VM name rather than using `${vm}` directly.
3. Create a private blob container such as `images`.
4. Upload the verified `.vhd` with `az storage blob upload --type page`.
   VHDs used as Azure image sources must be page blobs.
5. Create an Azure Compute Gallery in the same resource group.
6. Create its image definition with:
   - `--os-type Linux`
   - `--os-state Generalized`
   - `--architecture Arm64`
   - `--hyper-v-generation V2`
7. Create image version `3.24.1` from the VHD blob with
   `az sig image-version create --os-vhd-uri ...
   --os-vhd-storage-account ...`, targeting only `germanywestcentral` with one
   `Standard_LRS` replica.
8. Wait for the gallery image version's provisioning state to be `Succeeded`
   before creating the VM.

All storage accounts, blobs, gallery resources, networking resources, and the
VM must belong to `${vm}-rg`.

### 4. Generate the cloud-init configuration

Generate a temporary YAML file. Prefer cloud-init's declarative modules over a
large shell script where practical. It should:

- set `ssh_pwauth: false` and disable root SSH login;
- ensure `richard` is created with the requested public key and a locked
  password;
- add `richard` to the `wheel` group;
- write `/etc/sudoers.d/richard` as mode `0440` containing
  `richard ALL=(ALL:ALL) NOPASSWD: ALL`;
- run package index/update and install the minimal package list from the table;
- grow the root partition/filesystem to the chosen OS disk size;
- install rustup as `richard`, using the minimal profile;
- source `/home/richard/.cargo/env` for every rustup/Cargo command;
- run `rustup update` and `rustup target add wasm32-unknown-unknown`;
- run, as `richard`:

  ```sh
  cargo install --locked --force wasm-pack
  cargo install --locked terrazzo-css-cli
  cargo install --locked terrazzo-terminal --features prod
  ```

- run, only after the final Cargo installation has replaced the binary:

  ```sh
  setcap 'cap_net_bind_service=+ep' /home/richard/.cargo/bin/terrazzo-terminal
  ```

- leave an explicit success marker such as
  `/var/lib/terrazzo-provisioning-complete`; absence of the marker makes a
  partial cloud-init run easy to detect.

Do not put private keys, passwords, storage account keys, or other secrets in
cloud-init. The SSH public key is safe to include.

Use `--admin-username richard`, `--authentication-type ssh`, and the same
public key on `az vm create` as well. Let Azure's cloud-init provisioning path
create/configure the requested admin user; the custom cloud-config should
reinforce the locked-password, SSH, group, and sudo policy. Do not retain a
second usable `alpine` account from the base image.

### 5. Create the VM and network rules

Use the gallery image version ID with `az vm create` and include:

```text
--resource-group ${vm}-rg
--name $vm
--location germanywestcentral
--size Standard_D2ps_v6
--admin-username richard
--authentication-type ssh
--ssh-key-values <the public key above>
--custom-data <generated cloud-init file>
--enable-agent false
--security-type Standard
--public-ip-sku Standard
--os-disk-size-gb 32
--storage-sku Standard_LRS
```

A 32 GiB Standard HDD is the smallest normal Azure managed-disk billing tier
and gives Cargo enough working space; the official Alpine image itself remains
small. Ask Azure CLI to create the NIC, public IP, VNet/subnet, and NSG in the
same resource group. Set OS disk and NIC delete options to `Delete` if the
installed Azure CLI supports those flags, so deleting the VM does not orphan
them; this does not replace explicit resource-group cleanup.

Create NSG rules for inbound TCP 22, 80, and 443. Use separate, named rules
with deterministic priorities, or one SSH rule and one Web rule whose
destination port ranges are `80 443`. Restrict the source of port 22 if a
caller-configurable CIDR is added later; the stated requirements currently
imply Internet access for all three ports.

Opening an NSG port does not start `terrazzo-terminal`. This task installs the
binary and grants its executable permission to bind privileged ports; service
configuration, TLS certificates, and automatic startup are outside the stated
scope.

### 6. Report provisioning state clearly

Print the resource group, public IP, and SSH command. State explicitly that
first-boot installation may still be running. Provide these checks:

```sh
ssh richard@<public-ip> 'sudo cloud-init status --wait --long'
ssh richard@<public-ip> 'sudo tail -f /var/log/cloud-init-output.log'
```

If the script waits for completion itself, use SSH with `BatchMode=yes`, a
bounded retry loop for initial SSH availability, and then require both a
successful cloud-init status and the explicit success marker. Do not rely only
on `az vm create` returning success.

## Verification plan

### Static checks

- Run `bash -n terminal/scripts/create-azure-vm.sh`.
- Run `shellcheck terminal/scripts/create-azure-vm.sh` when ShellCheck is
  available.
- Review the generated cloud-init with `cloud-init schema --config-file` in an
  Alpine/cloud-init environment if possible.
- Confirm no secret/private-key material is present in the script or generated
  configuration.

### Disposable Azure smoke test

Run the script with a disposable VM name, then verify:

- `az resource list --resource-group "${vm}-rg"` shows every created resource
  in the required resource group.
- The VM reports size `Standard_D2ps_v6`, region `germanywestcentral`, and the
  expected gallery image version.
- `uname -m` returns `aarch64` and `/etc/alpine-release` returns the pinned
  Alpine release.
- Neither a `waagent` executable/service/process nor a WALinuxAgent package is
  present, and the VM model has `provisionVMAgent` set to false.
- `sshd -T` reports password authentication disabled and root login disabled.
- `passwd -S richard` reports a locked password.
- The supplied private key can log in as `richard`, while no `alpine` login
  account remains usable.
- `sudo -n true` succeeds for `richard`.
- `cloud-init status --wait --long` succeeds and the provisioning success
  marker exists.
- `rustc --version`, `cargo --version`, `protoc --version`, `wasm-pack
  --version`, `terrazzo-css --help`, and `terrazzo-terminal --help` succeed as
  `richard` (confirm the actual CLI name installed by `terrazzo-css-cli`).
- `rustup target list --installed` contains both the native
  `aarch64-unknown-linux-musl` target and `wasm32-unknown-unknown`.
- `getcap /home/richard/.cargo/bin/terrazzo-terminal` reports
  `cap_net_bind_service=ep`.
- The NSG allows TCP ports 22, 80, and 443 and no password-authentication rule
  or Azure extension was added.

After the smoke test, delete the disposable resource group explicitly with
`az group delete --name "${vm}-rg"`. Keep deletion out of the setup script.

## Sources consulted

- [Alpine official cloud images](https://www.alpinelinux.org/cloud/)
- [Alpine official Azure image directory](https://dl-cdn.alpinelinux.org/alpine/v3.24/releases/cloud/)
- [Alpine cloud-image import guidance](https://gitlab.alpinelinux.org/alpine/cloud/alpine-cloud-images/-/blob/main/IMPORTING.md)
- [Azure Dpsv6 size series](https://learn.microsoft.com/azure/virtual-machines/sizes/general-purpose/dpsv6-series)
- [Azure VMs without a provisioning agent](https://learn.microsoft.com/azure/virtual-machines/linux/no-agent)
- [Azure cloud-init support](https://learn.microsoft.com/azure/virtual-machines/linux/using-cloud-init)
- [Azure custom data](https://learn.microsoft.com/azure/virtual-machines/custom-data)
- [Azure CLI storage blob upload](https://learn.microsoft.com/cli/azure/storage/blob#az-storage-blob-upload)
- [Azure CLI Compute Gallery image definitions](https://learn.microsoft.com/cli/azure/sig/image-definition)
- [Alpine package index](https://pkgs.alpinelinux.org/packages)
