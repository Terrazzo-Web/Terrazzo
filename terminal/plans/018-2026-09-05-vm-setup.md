# Azure Linux 3 ARM64 VM setup plan

## Goal and scope

Plan a future `terminal/scripts/create-azure-vm.sh` script. Do not implement or
run the script as part of this task. The eventual interface will be:

```sh
terminal/scripts/create-azure-vm.sh <vm-name>
```

Assume `az login` has already completed. The script must create resources only
inside `${vm}-rg` and provision this VM:

- resource group: `${vm}-rg`
- region: `germanywestcentral` (Germany West Central)
- size: `Standard_D2ps_v6` (Azure Cobalt ARM64, 2 vCPUs, 8 GiB RAM)
- OS: Microsoft Azure Linux 3, ARM64 Marketplace image
- security: Trusted Launch with Secure Boot and vTPM enabled
- Azure Linux Agent/WALA: absent after first-boot provisioning, with the VM
  model configured not to expect an agent
- admin/login user: `richard`
- authentication: the supplied SSH public key only; no user or root password
  login
- authorization: passwordless `sudo` for `richard`
- inbound TCP ports: 22, 80, and 443
- installed tools: Rust, `wasm32-unknown-unknown`, `wasm-pack`,
  `terrazzo-css-cli`, `protoc`, and `terrazzo-terminal --features prod`

The public key is:

```text
ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBaRBRd4kNHeFgqxi09MlCNmf6DT4ELjoDDWNXqJUuJp Richard @ MacBook Air
```

## Architecture decisions

### Deploy the Microsoft Marketplace image directly

Use this image:

```text
Publisher: MicrosoftCBLMariner
Offer:     azure-linux-3
SKU:       azure-linux-3-arm64
Version:   3.20260809.01
URN:       MicrosoftCBLMariner:azure-linux-3:azure-linux-3-arm64:3.20260809.01
```

The version above was the concrete `latest` version in Germany West Central on
2026-09-05. Live Azure metadata confirmed that it is ARM64, Hyper-V generation
2, advertises `TrustedLaunchSupported`, and has no Marketplace purchase plan.
Pin the concrete version so repeated runs do not silently change the base OS.
Updating it later must be a deliberate plan/script change followed by another
smoke test.

This replaces the Alpine design completely. Do not download or upload a VHD,
create a storage account, create a Compute Gallery, or import/replicate an
image. `az vm create --image <URN>` can deploy Azure Linux directly and avoids
all custom-image resources.

Azure Linux is free of an additional OS license charge and this image has no
purchase plan. The Azure VM, managed disk, public IP, and network traffic still
incur normal subscription charges; “free” does not mean the VM is an Azure
free-tier SKU.

### Use Trusted Launch explicitly

Both halves of the deployment support Trusted Launch:

- the Azure Linux 3 ARM64 image is generation 2 and reports
  `TrustedLaunchSupported`;
- `Standard_D2ps_v6` supports Trusted Launch.

Pass all three settings rather than relying on CLI defaults:

```text
--security-type TrustedLaunch
--enable-secure-boot true
--enable-vtpm true
```

Secure Boot and vTPM are Azure platform/firmware features and remain active
without WALA. Guest Attestation and integrity monitoring use a VM extension,
however, so their portal reporting is intentionally unavailable on the final
agentless VM.

### Use cloud-init, then remove WALA

The Marketplace image includes WALA. `--enable-agent false` changes the Azure
VM model but does not uninstall the package from the image. Therefore the
cloud-init payload must stop, disable, and uninstall `WALinuxAgent` before the
rest of the custom provisioning.

This has one important semantic consequence: WALA exists in the base image and
may run briefly during the first boot before cloud-init removes it. The required
steady state is fully agentless. Producing an image in which WALA never exists
would require a temporary builder VM plus a generalized Compute Gallery image;
that extra image pipeline is not justified for this setup.

Create the VM with `--enable-agent false`. Do not install any Azure monitoring,
management, Guest Attestation, or other VM extension. After removal:

- VM extensions and `az vm run-command` are unavailable;
- extension-based key/password recovery and Guest Attestation are unavailable;
- Secure Boot and vTPM remain enabled;
- diagnostics use SSH, cloud-init logs, boot diagnostics, or the serial console.

`az vm create` can finish before cloud-init completes the lengthy Cargo builds,
so Azure deployment success is not provisioning success.

## Ubuntu/Alpine command translation for Azure Linux 3

Azure Linux is RPM/glibc based and uses `tdnf`. Do not carry forward either
Ubuntu `apt` commands or Alpine `apk`/musl assumptions.

| Ubuntu or Alpine concept | Azure Linux 3 command/package | Notes |
| --- | --- | --- |
| `apt update` / `apk update` | `tdnf makecache` | Refresh repository metadata. |
| `apt dist-upgrade` / `apk upgrade` | `tdnf update -y` | Apply available package updates. |
| `build-essential` / `build-base` | `build-essential` | Azure Linux provides this build-tool metapackage. |
| `pkg-config` / `pkgconf` | `pkgconf-pkg-config` | Provides the `pkg-config` command. |
| `musl-tools` | none | Azure Linux is glibc based; use its native GNU target. |
| `libssl-dev` / `openssl-dev` | `openssl-devel` | OpenSSL headers and metadata. |
| `protobuf-compiler` / `protoc` | `protobuf` | Provides `protoc` and the `protobuf-compiler` capability. |
| `libprotobuf-dev` / `protobuf-dev` | `protobuf-devel` | Development headers and metadata. |
| `libcap2-bin` / `libcap-setcap` | `libcap` | Provides `setcap`/`getcap`. |
| `curl`, `nano`, `sudo` | same package names | Available in the ARM64 base repository. |
| `walinuxagent` | `WALinuxAgent` | Remove this package; do not install it. |

The package names above were checked against the official Azure Linux 3 ARM64
base-repository metadata. Install the explicit minimum set:

```text
build-essential
pkgconf-pkg-config
openssl-devel
protobuf
protobuf-devel
libcap
curl
nano
sudo
ca-certificates
```

On this host, rustup's native target is `aarch64-unknown-linux-gnu`. Add only
the extra WASM target, `wasm32-unknown-unknown`.

The `terrazzo-css-cli` package installs the `terrazzo-css` command.

## Future script implementation

### 1. Validate inputs and Azure state

- Use `#!/usr/bin/env bash` and `set -euo pipefail`.
- Require exactly one positional argument, assign it to `vm`, and derive
  `resource_group="${vm}-rg"`.
- Validate `vm` before interpolating it into Azure resource names.
- Require `az`, `ssh`, and the local utilities used for temporary files.
- Confirm an active account/subscription with `az account show`.
- Refuse to continue if `${vm}-rg` already exists. This avoids mutating an
  unrelated or partially completed deployment.
- Query `az vm list-skus` for `Standard_D2ps_v6` in `germanywestcentral` and
  fail clearly if it is unavailable or restricted for the active subscription.
- Run `az vm image show` for the pinned URN in the target region and require:
  `architecture == Arm64`, `hyperVGeneration == V2`, the security feature
  contains `TrustedLaunchSupported`, and `plan == null`.
- Use a cleanup trap only for local temporary files. Preserve failed Azure
  resources for diagnosis; never auto-delete the resource group on failure.

### 2. Generate cloud-init configuration

Write a temporary `#cloud-config` file. It should:

- set `ssh_pwauth: false` and disable root SSH login;
- create or update `richard`, install only the supplied public key, and lock the
  password;
- put `richard` in the `wheel` administrator group and write
  `/etc/sudoers.d/richard` with mode `0440` and:

  ```text
  richard ALL=(ALL:ALL) NOPASSWD: ALL
  ```

- stop and disable any `walinuxagent.service`/`waagent.service` unit that is
  present, then run `tdnf remove -y WALinuxAgent`;
- refresh/update packages with `tdnf` and install the minimum package set above;
- install rustup as `richard` with the minimal profile;
- source `/home/richard/.cargo/env` for every rustup/Cargo command;
- run `rustup update` and `rustup target add wasm32-unknown-unknown`;
- run as `richard`:

  ```sh
  cargo install --locked --force wasm-pack
  cargo install --locked terrazzo-css-cli
  cargo install --locked terrazzo-terminal --features prod
  ```

- after the final Cargo command has installed the final binary, run:

  ```sh
  setcap 'cap_net_bind_service=+ep' /home/richard/.cargo/bin/terrazzo-terminal
  ```

- verify the expected commands and capability, and only then create
  `/var/lib/terrazzo-provisioning-complete`.

Use a shell command for conditional service removal because the image/package
may expose either WALA unit name. Do not hide failure of `tdnf remove`; the
success marker must never be written while the package remains installed.

Do not put a private key, password, access token, storage credential, or other
secret in custom data. Azure retains custom data in VM metadata; the supplied
public key is safe to include.

### 3. Create the resource group and VM

Create `${vm}-rg` in `germanywestcentral`, then use a single direct Marketplace
deployment. The core `az vm create` arguments are:

```text
--resource-group ${vm}-rg
--name $vm
--location germanywestcentral
--image MicrosoftCBLMariner:azure-linux-3:azure-linux-3-arm64:3.20260809.01
--size Standard_D2ps_v6
--admin-username richard
--authentication-type ssh
--ssh-key-values <the public key above>
--custom-data <generated cloud-init file>
--enable-agent false
--security-type TrustedLaunch
--enable-secure-boot true
--enable-vtpm true
--public-ip-sku Standard
--os-disk-size-gb 32
--storage-sku Standard_LRS
```

Use a 32 GiB Standard HDD OS disk: Azure Linux itself is small, while Rust/Cargo
builds need working space. `Standard_D2ps_v6` has no local temporary disk, so do
not plan to place build artifacts on an ephemeral disk.

Let Azure CLI create the VNet, subnet, NIC, public IP, and NSG inside the same
resource group. Set OS-disk, NIC, and public-IP delete options to `Delete` where
the installed CLI supports them so VM deletion does not leave incidental
resources behind. Resource-group deletion remains the explicit full cleanup.

### 4. Configure ingress

Create deterministic NSG rules for:

- TCP 22 (SSH)
- TCP 80 (HTTP)
- TCP 443 (HTTPS)

The stated requirements imply Internet sources for all three. A future optional
SSH source-CIDR argument would be a useful hardening change, but it must not be
invented as a second required positional argument.

An NSG rule only permits traffic. This scope installs `terrazzo-terminal` and
grants it permission to bind privileged ports; it does not create a systemd
service, choose application arguments, configure TLS certificates, or start the
application.

### 5. Report asynchronous provisioning honestly

Print the resource group, public IP, and SSH command, and state that first-boot
provisioning may still be running. The operator checks it with:

```sh
ssh richard@<public-ip> 'sudo cloud-init status --wait --long'
ssh richard@<public-ip> 'sudo tail -f /var/log/cloud-init-output.log'
```

If the script waits automatically, use `ssh -o BatchMode=yes` with a bounded
retry loop, then require both successful cloud-init status and the explicit
success marker. Do not use `az vm run-command`, because that requires WALA.

## Verification plan

### Static validation after the script is implemented

- Run `bash -n terminal/scripts/create-azure-vm.sh`.
- Run `shellcheck terminal/scripts/create-azure-vm.sh` when available.
- Validate the generated YAML with `cloud-init schema --config-file` in a
  compatible environment.
- Inspect generated custom data for accidental secrets.
- Confirm the script contains no VHD, storage-account, Compute Gallery, Alpine,
  `apk`, `apt`, or musl-image workflow remnants.

### Disposable Azure smoke test

Use a disposable VM name and verify:

- every created resource returned by `az resource list --resource-group
  "${vm}-rg"` belongs to the requested group;
- the VM is in `germanywestcentral`, uses `Standard_D2ps_v6`, and references the
  pinned Azure Linux image version;
- `az vm show` reports `securityType: TrustedLaunch`, `secureBootEnabled: true`,
  `vTpmEnabled: true`, and `provisionVMAgent: false`;
- `uname -m` returns `aarch64`, `/etc/os-release` identifies Azure Linux 3, and
  `/sys/firmware/efi` exists;
- the `WALinuxAgent` RPM, WALA executable, systemd unit, and process are absent;
- `az vm extension list` is empty;
- cloud-init completed successfully and
  `/var/lib/terrazzo-provisioning-complete` exists;
- SSH password authentication and root login are disabled, `richard`'s password
  is locked, key login works, and `sudo -n true` succeeds;
- `rustc --version`, `cargo --version`, `protoc --version`, `wasm-pack
  --version`, `terrazzo-css --help`, and `terrazzo-terminal --help` all succeed
  as `richard`;
- `rustc -vV` reports an `aarch64-unknown-linux-gnu` host and
  `rustup target list --installed` includes `wasm32-unknown-unknown`;
- `getcap /home/richard/.cargo/bin/terrazzo-terminal` reports
  `cap_net_bind_service=ep`;
- the NSG permits inbound TCP 22, 80, and 443 and no additional application
  port was opened.

After verification, explicitly delete the disposable resource group with
`az group delete --name "${vm}-rg"`. Cleanup does not belong in the setup
script itself.

## Sources consulted

- [Azure Linux VM overview](https://learn.microsoft.com/azure/azure-linux/azure-linux-vm-vmss-overview)
- [Azure Linux package repository](https://packages.microsoft.com/azurelinux/3.0/prod/base/aarch64/)
- [Azure Dpsv6 size series](https://learn.microsoft.com/azure/virtual-machines/sizes/general-purpose/dpsv6-series)
- [Trusted Launch for Azure VMs](https://learn.microsoft.com/azure/virtual-machines/trusted-launch)
- [Trusted Launch FAQ](https://learn.microsoft.com/azure/virtual-machines/trusted-launch-faq)
- [Azure VMs without a provisioning agent](https://learn.microsoft.com/azure/virtual-machines/linux/disable-provisioning)
- [Cloud-init support for Azure VMs](https://learn.microsoft.com/azure/virtual-machines/linux/using-cloud-init)
- [Azure VM custom data](https://learn.microsoft.com/azure/virtual-machines/custom-data)
- [Azure CLI `az vm create`](https://learn.microsoft.com/cli/azure/vm#az-vm-create)
