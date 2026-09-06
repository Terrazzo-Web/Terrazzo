#!/usr/bin/env bash

set -euo pipefail

readonly location="germanywestcentral"
readonly vm_size="Standard_D2ps_v6"
readonly image_urn="MicrosoftCBLMariner:azure-linux-3:azure-linux-3-arm64:3.20260809.01"
readonly admin_username="richard"
readonly ssh_public_key="ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBaRBRd4kNHeFgqxi09MlCNmf6DT4ELjoDDWNXqJUuJp Richard @ MacBook Air"

usage() {
  echo "Usage: $0 <vm-name>" >&2
}

fail() {
  echo "Error: $*" >&2
  exit 1
}

if (( $# != 1 )); then
  usage
  exit 2
fi

readonly vm="$1"

# Keep the name conservative because it is also the prefix for network
# resources with their own length and character restrictions.
if [[ ! "$vm" =~ ^[A-Za-z0-9]([A-Za-z0-9-]{0,48}[A-Za-z0-9])?$ ]]; then
  fail "VM name must be 1-50 characters, contain only letters, numbers, and hyphens, and start and end with a letter or number"
fi

readonly resource_group="${vm}-rg"
readonly vnet_name="${vm}-vnet"
readonly subnet_name="${vm}-subnet"
readonly nsg_name="${vm}-nsg"
readonly public_ip_name="${vm}-ip"

for command_name in az mktemp rm ssh; do
  command -v "$command_name" >/dev/null 2>&1 || fail "required command not found: $command_name"
done

cloud_init_file="$(mktemp "${TMPDIR:-/tmp}/terrazzo-cloud-init.XXXXXX")"

cleanup() {
  rm -f -- "$cloud_init_file"
}
trap cleanup EXIT

echo "Checking Azure account..."
az account show --output none

if [[ "$(az group exists --name "$resource_group" --output tsv)" == "true" ]]; then
  fail "resource group $resource_group already exists; refusing to modify it"
fi

echo "Checking VM size availability..."
available_sku_count="$(
  az vm list-skus \
    --location "$location" \
    --size "$vm_size" \
    --resource-type virtualMachines \
    --all \
    --query "[?name == '$vm_size' && length(restrictions) == \`0\`] | length(@)" \
    --output tsv
)"
if [[ "$available_sku_count" == "0" ]]; then
  fail "$vm_size is unavailable or restricted for this subscription in $location"
fi

echo "Checking pinned image metadata..."
image_architecture="$(
  az vm image show \
    --location "$location" \
    --urn "$image_urn" \
    --query architecture \
    --output tsv
)"
[[ "$image_architecture" == "Arm64" ]] || fail "image architecture is $image_architecture, expected Arm64"

image_generation="$(
  az vm image show \
    --location "$location" \
    --urn "$image_urn" \
    --query hyperVGeneration \
    --output tsv
)"
[[ "$image_generation" == "V2" ]] || fail "image generation is $image_generation, expected V2"

image_security="$(
  az vm image show \
    --location "$location" \
    --urn "$image_urn" \
    --query "features[?name == 'SecurityType'].value | [0]" \
    --output tsv
)"
[[ "$image_security" == "TrustedLaunchSupported" ]] || fail "image does not advertise Trusted Launch support"

image_plan="$(
  az vm image show \
    --location "$location" \
    --urn "$image_urn" \
    --query plan \
    --output json
)"
[[ -z "$image_plan" || "$image_plan" == "null" ]] || fail "image unexpectedly requires a Marketplace purchase plan"

cat >"$cloud_init_file" <<'CLOUD_INIT'
#cloud-config
ssh_pwauth: false
disable_root: true

users:
  - name: richard
    gecos: Richard
    groups:
      - wheel
    shell: /bin/bash
    lock_passwd: true
    sudo:
      - "ALL=(ALL:ALL) NOPASSWD: ALL"
    ssh_authorized_keys:
      - ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBaRBRd4kNHeFgqxi09MlCNmf6DT4ELjoDDWNXqJUuJp Richard @ MacBook Air

write_files:
  - path: /etc/sudoers.d/richard
    owner: root:root
    permissions: '0440'
    content: |
      richard ALL=(ALL:ALL) NOPASSWD: ALL
  - path: /usr/local/sbin/provision-terrazzo
    owner: root:root
    permissions: '0700'
    content: |
      #!/usr/bin/env bash
      set -euxo pipefail

      for unit in walinuxagent.service waagent.service; do
        if systemctl list-unit-files "$unit" --no-legend 2>/dev/null | grep -q "^${unit}"; then
          systemctl disable --now "$unit"
        fi
      done

      if rpm -q WALinuxAgent >/dev/null 2>&1; then
        tdnf remove -y WALinuxAgent
      fi
      systemctl daemon-reload

      tdnf makecache
      tdnf update -y
      tdnf install -y \
        build-essential \
        ca-certificates \
        curl \
        libcap \
        nano \
        openssl-devel \
        pkgconf-pkg-config \
        protobuf \
        protobuf-devel \
        sudo

      usermod --append --groups wheel richard
      passwd --lock richard
      visudo --check --file=/etc/sudoers.d/richard

      sed -i -E \
        -e 's/^[#[:space:]]*PasswordAuthentication[[:space:]].*/PasswordAuthentication no/' \
        -e 's/^[#[:space:]]*KbdInteractiveAuthentication[[:space:]].*/KbdInteractiveAuthentication no/' \
        -e 's/^[#[:space:]]*PermitRootLogin[[:space:]].*/PermitRootLogin no/' \
        -e 's/^[#[:space:]]*PubkeyAuthentication[[:space:]].*/PubkeyAuthentication yes/' \
        /etc/ssh/sshd_config
      sshd -t
      systemctl restart sshd

      sudo --user=richard --set-home bash -c \
        'curl --proto "=https" --tlsv1.2 --silent --show-error --fail https://sh.rustup.rs | sh -s -- -y --profile minimal'

      sudo --user=richard --set-home bash -c '
        set -euxo pipefail
        source "$HOME/.cargo/env"
        rustup update
        rustup target add wasm32-unknown-unknown
        cargo install --locked --force wasm-pack
        cargo install --locked terrazzo-css-cli
        cargo install --locked terrazzo-terminal --features prod
      '

      setcap 'cap_net_bind_service=+ep' /home/richard/.cargo/bin/terrazzo-terminal

      test "$(uname -m)" = aarch64
      grep -q '^ID=azurelinux$' /etc/os-release
      ! rpm -q WALinuxAgent >/dev/null 2>&1
      ! command -v waagent >/dev/null 2>&1
      for unit in walinuxagent.service waagent.service; do
        ! systemctl is-active --quiet "$unit"
      done
      ! pgrep -f '[w]aagent' >/dev/null
      sudo --user=richard --set-home /home/richard/.cargo/bin/rustc --version
      sudo --user=richard --set-home /home/richard/.cargo/bin/cargo --version
      sudo --user=richard --set-home /home/richard/.cargo/bin/rustup target list --installed | grep -q '^wasm32-unknown-unknown'
      protoc --version
      sudo --user=richard --set-home /home/richard/.cargo/bin/wasm-pack --version
      sudo --user=richard --set-home /home/richard/.cargo/bin/terrazzo-css --help
      sudo --user=richard --set-home /home/richard/.cargo/bin/terrazzo-terminal --help
      getcap /home/richard/.cargo/bin/terrazzo-terminal | grep -q 'cap_net_bind_service=ep'

      touch /var/lib/terrazzo-provisioning-complete

runcmd:
  - /usr/local/sbin/provision-terrazzo
CLOUD_INIT

echo "Creating resource group $resource_group..."
az group create \
  --name "$resource_group" \
  --location "$location" \
  --output none

echo "Creating Trusted Launch ARM64 VM $vm..."
public_ip="$(
  az vm create \
    --resource-group "$resource_group" \
    --name "$vm" \
    --location "$location" \
    --image "$image_urn" \
    --size "$vm_size" \
    --admin-username "$admin_username" \
    --authentication-type ssh \
    --ssh-key-values "$ssh_public_key" \
    --custom-data "$cloud_init_file" \
    --enable-agent false \
    --enable-auto-update false \
    --security-type TrustedLaunch \
    --enable-secure-boot true \
    --enable-vtpm true \
    --os-disk-size-gb 32 \
    --storage-sku Standard_LRS \
    --os-disk-delete-option Delete \
    --vnet-name "$vnet_name" \
    --subnet "$subnet_name" \
    --nsg "$nsg_name" \
    --nsg-rule NONE \
    --nic-delete-option Delete \
    --public-ip-address "$public_ip_name" \
    --public-ip-address-allocation static \
    --public-ip-sku Standard \
    --query publicIpAddress \
    --output tsv
)"

echo "Creating inbound network rules..."
az network nsg rule create \
  --resource-group "$resource_group" \
  --nsg-name "$nsg_name" \
  --name AllowSsh \
  --priority 100 \
  --direction Inbound \
  --access Allow \
  --protocol Tcp \
  --source-address-prefixes Internet \
  --source-port-ranges '*' \
  --destination-address-prefixes '*' \
  --destination-port-ranges 22 \
  --output none

az network nsg rule create \
  --resource-group "$resource_group" \
  --nsg-name "$nsg_name" \
  --name AllowWeb \
  --priority 110 \
  --direction Inbound \
  --access Allow \
  --protocol Tcp \
  --source-address-prefixes Internet \
  --source-port-ranges '*' \
  --destination-address-prefixes '*' \
  --destination-port-ranges 80 443 \
  --output none

if [[ -z "$public_ip" ]]; then
  public_ip="$(
    az network public-ip show \
      --resource-group "$resource_group" \
      --name "$public_ip_name" \
      --query ipAddress \
      --output tsv
  )"
fi

echo
echo "VM deployment completed. First-boot provisioning may still be running."
echo "Resource group: $resource_group"
echo "Public IP:     $public_ip"
echo "SSH:           ssh ${admin_username}@${public_ip}"
echo
echo "Wait for cloud-init:"
echo "  ssh ${admin_username}@${public_ip} 'sudo cloud-init status --wait --long'"
echo "Follow provisioning output:"
echo "  ssh ${admin_username}@${public_ip} 'sudo tail -f /var/log/cloud-init-output.log'"
