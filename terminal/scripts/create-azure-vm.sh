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
readonly nic_name="${vm}-nic"
readonly public_ip_name="${vm}-ip"
readonly load_balancer_name="${vm}-lb"
readonly frontend_ip_name="${vm}-frontend"
readonly backend_pool_name="${vm}-backend"
readonly health_probe_name="${vm}-ssh-probe"

for command_name in az mktemp rm ssh; do
  command -v "$command_name" >/dev/null 2>&1 || fail "required command not found: $command_name"
done

cloud_init_file="$(mktemp "${TMPDIR:-/tmp}/terrazzo-cloud-init.XXXXXX")"

cleanup() {
  rm -f -- "$cloud_init_file"
}
trap cleanup EXIT

echo "Checking Azure account..."
# Verify that Azure CLI has an active authenticated subscription.
# --output: Suppress account details because only command success is needed.
az account show --output none

# Check whether the derived resource-group name is already in use.
# --name: Select the resource group derived from the requested VM name.
# --output: Return the Boolean result as unquoted tab-separated text.
resource_group_exists="$(az group exists --name "$resource_group" --output tsv)"
if [[ "$resource_group_exists" == "true" ]]; then
  fail "resource group $resource_group already exists; refusing to modify it"
fi

echo "Checking VM size availability..."
available_sku_count="$(
  # Count unrestricted instances of the selected VM SKU in the target region.
  # --location: Limit the SKU search to the deployment region.
  # --size: Ask Azure for the selected VM-size family.
  # --resource-type: Return only virtual-machine SKUs.
  # --all: Include all matching SKUs rather than the default CLI subset.
  # --query: Count exact SKU matches that have no subscription restrictions.
  # --output: Return the count as unquoted tab-separated text.
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
  # Read the CPU architecture advertised by the pinned Marketplace image.
  # --location: Resolve image metadata in the deployment region.
  # --urn: Select the exact pinned publisher, offer, SKU, and version.
  # --query: Extract only the image architecture field.
  # --output: Return the architecture as unquoted tab-separated text.
  az vm image show \
    --location "$location" \
    --urn "$image_urn" \
    --query architecture \
    --output tsv
)"
[[ "$image_architecture" == "Arm64" ]] || fail "image architecture is $image_architecture, expected Arm64"

image_generation="$(
  # Read the Hyper-V generation advertised by the pinned Marketplace image.
  # --location: Resolve image metadata in the deployment region.
  # --urn: Select the exact pinned publisher, offer, SKU, and version.
  # --query: Extract only the Hyper-V generation field.
  # --output: Return the generation as unquoted tab-separated text.
  az vm image show \
    --location "$location" \
    --urn "$image_urn" \
    --query hyperVGeneration \
    --output tsv
)"
[[ "$image_generation" == "V2" ]] || fail "image generation is $image_generation, expected V2"

image_security="$(
  # Read the security capability advertised by the pinned Marketplace image.
  # --location: Resolve image metadata in the deployment region.
  # --urn: Select the exact pinned publisher, offer, SKU, and version.
  # --query: Extract the first SecurityType feature value.
  # --output: Return the security capability as unquoted tab-separated text.
  az vm image show \
    --location "$location" \
    --urn "$image_urn" \
    --query "features[?name == 'SecurityType'].value | [0]" \
    --output tsv
)"
[[ "$image_security" == "TrustedLaunchSupported" ]] || fail "image does not advertise Trusted Launch support"

image_plan="$(
  # Check whether the pinned Marketplace image requires a purchase plan.
  # --location: Resolve image metadata in the deployment region.
  # --urn: Select the exact pinned publisher, offer, SKU, and version.
  # --query: Extract only the Marketplace plan object.
  # --output: Preserve the plan value as JSON so null is unambiguous.
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
# Create the resource-group boundary that owns the complete deployment.
# --name: Assign the resource group derived from the VM name.
# --location: Store the resource-group metadata in the deployment region.
# --output: Suppress the returned resource-group document.
az group create \
  --name "$resource_group" \
  --location "$location" \
  --output none

echo "Creating virtual network and network security group..."
# Create the virtual network and its single backend subnet.
# --resource-group: Place the network in this deployment's resource group.
# --name: Assign the derived virtual-network name.
# --location: Deploy the network in the selected Azure region.
# --address-prefixes: Reserve this private IPv4 range for the virtual network.
# --subnet-name: Create the named backend subnet with the virtual network.
# --subnet-prefixes: Allocate this private IPv4 range to the backend subnet.
# --no-wait: Submit the long-running operation without using the faulty CLI poller.
# --output: Suppress the initial operation response.
az network vnet create \
  --resource-group "$resource_group" \
  --name "$vnet_name" \
  --location "$location" \
  --address-prefixes 10.0.0.0/16 \
  --subnet-name "$subnet_name" \
  --subnet-prefixes 10.0.0.0/24 \
  --no-wait \
  --output none
# Wait until Azure reports that the virtual network was created successfully.
# --resource-group: Find the virtual network in this deployment's resource group.
# --name: Select the derived virtual-network name.
# --created: Wait for a successful Created provisioning state.
az network vnet wait \
  --resource-group "$resource_group" \
  --name "$vnet_name" \
  --created

# Create the network security group applied to the VM's network interface.
# --resource-group: Place the NSG in this deployment's resource group.
# --name: Assign the derived NSG name.
# --location: Deploy the NSG in the selected Azure region.
# --no-wait: Submit the long-running operation without using the faulty CLI poller.
# --output: Suppress the initial operation response.
az network nsg create \
  --resource-group "$resource_group" \
  --name "$nsg_name" \
  --location "$location" \
  --no-wait \
  --output none
# Wait until Azure reports that the network security group was created.
# --resource-group: Find the NSG in this deployment's resource group.
# --name: Select the derived NSG name.
# --created: Wait for a successful Created provisioning state.
az network nsg wait \
  --resource-group "$resource_group" \
  --name "$nsg_name" \
  --created

echo "Creating inbound network security rules..."
# Permit inbound SSH traffic to the backend VM.
# --resource-group: Find the NSG in this deployment's resource group.
# --nsg-name: Add the rule to the derived NSG.
# --name: Give the SSH rule a stable name.
# --priority: Evaluate this allow rule before rules with higher priority numbers.
# --direction: Apply the rule to traffic entering the NIC.
# --access: Permit traffic that matches the rule.
# --protocol: Match only TCP traffic.
# --source-address-prefixes: Accept traffic originating on the public Internet.
# --source-port-ranges: Accept any client-side source port.
# --destination-address-prefixes: Match any private IP configuration on the NIC.
# --destination-port-ranges: Match the SSH service port.
# --no-wait: Submit the rule update without using the faulty CLI poller.
# --output: Suppress the initial operation response.
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
  --no-wait \
  --output none
# Wait until Azure reports that the SSH rule update is complete.
# --resource-group: Find the NSG in this deployment's resource group.
# --name: Select the derived NSG name.
# --updated: Wait for a successful Updated provisioning state.
az network nsg wait \
  --resource-group "$resource_group" \
  --name "$nsg_name" \
  --updated

# Permit inbound HTTP and HTTPS traffic to the backend VM.
# --resource-group: Find the NSG in this deployment's resource group.
# --nsg-name: Add the rule to the derived NSG.
# --name: Give the web rule a stable name.
# --priority: Evaluate this rule after SSH and before Azure's default deny rule.
# --direction: Apply the rule to traffic entering the NIC.
# --access: Permit traffic that matches the rule.
# --protocol: Match only TCP traffic.
# --source-address-prefixes: Accept traffic originating on the public Internet.
# --source-port-ranges: Accept any client-side source port.
# --destination-address-prefixes: Match any private IP configuration on the NIC.
# --destination-port-ranges: Match the standard HTTP and HTTPS service ports.
# --no-wait: Submit the rule update without using the faulty CLI poller.
# --output: Suppress the initial operation response.
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
  --no-wait \
  --output none
# Wait until Azure reports that the web rule update is complete.
# --resource-group: Find the NSG in this deployment's resource group.
# --name: Select the derived NSG name.
# --updated: Wait for a successful Updated provisioning state.
az network nsg wait \
  --resource-group "$resource_group" \
  --name "$nsg_name" \
  --updated

echo "Creating Standard public load balancer..."
# Reserve the static Standard public IPv4 address used by the load balancer.
# --resource-group: Place the public IP in this deployment's resource group.
# --name: Assign the derived public-IP resource name.
# --location: Allocate the address in the selected Azure region.
# --sku: Make the address compatible with a Standard Load Balancer.
# --allocation-method: Keep the assigned address stable for the resource lifetime.
# --output: Suppress the returned public-IP document.
az network public-ip create \
  --resource-group "$resource_group" \
  --name "$public_ip_name" \
  --location "$location" \
  --sku Standard \
  --allocation-method Static \
  --output none
# Wait until Azure reports that the public IP was created successfully.
# --resource-group: Find the public IP in this deployment's resource group.
# --name: Select the derived public-IP resource name.
# --created: Wait for a successful Created provisioning state.
az network public-ip wait \
  --resource-group "$resource_group" \
  --name "$public_ip_name" \
  --created

# Create the Standard public load balancer, frontend, and empty backend pool.
# --resource-group: Place the load balancer in this deployment's resource group.
# --name: Assign the derived load-balancer name.
# --location: Deploy the load balancer in the selected Azure region.
# --sku: Use the production Standard Load Balancer SKU.
# --public-ip-address: Attach the reserved public IP to the frontend.
# --frontend-ip-name: Name the load balancer's public frontend configuration.
# --backend-pool-name: Create the pool that will contain the VM's private NIC.
# --no-wait: Submit the long-running operation without using the faulty CLI poller.
# --output: Suppress the initial operation response.
az network lb create \
  --resource-group "$resource_group" \
  --name "$load_balancer_name" \
  --location "$location" \
  --sku Standard \
  --public-ip-address "$public_ip_name" \
  --frontend-ip-name "$frontend_ip_name" \
  --backend-pool-name "$backend_pool_name" \
  --no-wait \
  --output none
# Wait until Azure reports that the load balancer was created successfully.
# --resource-group: Find the load balancer in this deployment's resource group.
# --name: Select the derived load-balancer name.
# --created: Wait for a successful Created provisioning state.
az network lb wait \
  --resource-group "$resource_group" \
  --name "$load_balancer_name" \
  --created

# Add a TCP health probe that treats an accepting SSH service as healthy.
# --resource-group: Find the load balancer in this deployment's resource group.
# --lb-name: Add the probe to the derived load balancer.
# --name: Assign the derived probe name.
# --protocol: Test backend health with TCP.
# --port: Connect to the backend SSH port for each health check.
# --no-wait: Submit the probe update without using the faulty CLI poller.
# --output: Suppress the initial operation response.
az network lb probe create \
  --resource-group "$resource_group" \
  --lb-name "$load_balancer_name" \
  --name "$health_probe_name" \
  --protocol Tcp \
  --port 22 \
  --no-wait \
  --output none
# Wait until Azure reports that the health-probe update is complete.
# --resource-group: Find the load balancer in this deployment's resource group.
# --name: Select the derived load-balancer name.
# --updated: Wait for a successful Updated provisioning state.
az network lb wait \
  --resource-group "$resource_group" \
  --name "$load_balancer_name" \
  --updated

for port in 22 80 443; do
  # Forward this public TCP service port to the same port on a healthy backend.
  # --resource-group: Find the load balancer in this deployment's resource group.
  # --lb-name: Add the rule to the derived load balancer.
  # --name: Give the rule a stable name containing its TCP port.
  # --protocol: Match TCP traffic only.
  # --frontend-ip-name: Listen on the named public frontend configuration.
  # --frontend-port: Listen on the current public service port.
  # --backend-pool-name: Distribute matching traffic to the VM backend pool.
  # --backend-port: Preserve the port number when forwarding to the VM.
  # --probe-name: Send traffic only to backends passing the SSH health probe.
  # --disable-outbound-snat: Reserve outbound translation for the explicit outbound rule.
  # --enable-tcp-reset: Reset idle or unexpectedly terminated TCP connections cleanly.
  # --idle-timeout: Keep inactive TCP flows for up to 15 minutes.
  # --no-wait: Submit the rule update without using the faulty CLI poller.
  # --output: Suppress the initial operation response.
  az network lb rule create \
    --resource-group "$resource_group" \
    --lb-name "$load_balancer_name" \
    --name "Tcp${port}" \
    --protocol Tcp \
    --frontend-ip-name "$frontend_ip_name" \
    --frontend-port "$port" \
    --backend-pool-name "$backend_pool_name" \
    --backend-port "$port" \
    --probe-name "$health_probe_name" \
    --disable-outbound-snat true \
    --enable-tcp-reset true \
    --idle-timeout 15 \
    --no-wait \
    --output none
  # Wait until Azure reports that the current load-balancing rule is active.
  # --resource-group: Find the load balancer in this deployment's resource group.
  # --name: Select the derived load-balancer name.
  # --updated: Wait for a successful Updated provisioning state.
  az network lb wait \
    --resource-group "$resource_group" \
    --name "$load_balancer_name" \
    --updated
done

# Create explicit TCP and UDP Internet SNAT for every VM in the backend pool.
# --resource-group: Find the load balancer in this deployment's resource group.
# --lb-name: Add the outbound rule to the derived load balancer.
# --name: Give the outbound rule a stable descriptive name.
# --protocol: Provide outbound translation for both TCP and UDP.
# --frontend-ip-configs: Use the public frontend address as the translated source.
# --address-pool: Apply outbound translation to members of the VM backend pool.
# --allocated-outbound-ports: Reserve 10,000 SNAT ports for each backend instance.
# --idle-timeout: Retain inactive translated flows for up to 15 minutes.
# --no-wait: Submit the rule update without using the faulty CLI poller.
# --output: Suppress the initial operation response.
az network lb outbound-rule create \
  --resource-group "$resource_group" \
  --lb-name "$load_balancer_name" \
  --name InternetOutbound \
  --protocol All \
  --frontend-ip-configs "$frontend_ip_name" \
  --address-pool "$backend_pool_name" \
  --allocated-outbound-ports 10000 \
  --idle-timeout 15 \
  --no-wait \
  --output none
# Wait until Azure reports that the outbound-rule update is complete.
# --resource-group: Find the load balancer in this deployment's resource group.
# --name: Select the derived load-balancer name.
# --updated: Wait for a successful Updated provisioning state.
az network lb wait \
  --resource-group "$resource_group" \
  --name "$load_balancer_name" \
  --updated

echo "Creating load balancer backend network interface..."
# Create the VM's private NIC and enroll it in the load balancer backend pool.
# --resource-group: Place the NIC in this deployment's resource group.
# --name: Assign the derived NIC name.
# --location: Deploy the NIC in the selected Azure region.
# --vnet-name: Connect the NIC to the deployment's virtual network.
# --subnet: Place the NIC in the backend subnet.
# --network-security-group: Apply the deployment's inbound filtering rules.
# --lb-name: Select the load balancer that owns the backend pool.
# --lb-address-pools: Add the NIC's primary IP configuration to the backend pool.
# --no-wait: Submit the long-running operation without using the faulty CLI poller.
# --output: Suppress the initial operation response.
az network nic create \
  --resource-group "$resource_group" \
  --name "$nic_name" \
  --location "$location" \
  --vnet-name "$vnet_name" \
  --subnet "$subnet_name" \
  --network-security-group "$nsg_name" \
  --lb-name "$load_balancer_name" \
  --lb-address-pools "$backend_pool_name" \
  --no-wait \
  --output none
# Wait until Azure reports that the backend NIC was created successfully.
# --resource-group: Find the NIC in this deployment's resource group.
# --name: Select the derived NIC name.
# --created: Wait for a successful Created provisioning state.
az network nic wait \
  --resource-group "$resource_group" \
  --name "$nic_name" \
  --created

echo "Creating Trusted Launch ARM64 VM $vm behind $load_balancer_name..."
# Create the agentless Trusted Launch ARM64 VM using the prepared private NIC.
# --resource-group: Place the VM in this deployment's resource group.
# --name: Assign the user-supplied VM name.
# --location: Deploy the VM in the selected Azure region.
# --image: Install the exact pinned Azure Linux 3 ARM64 image.
# --size: Allocate the selected ARM64 CPU and memory SKU.
# --admin-username: Create the named administrative login account.
# --authentication-type: Permit SSH key authentication rather than passwords.
# --ssh-key-values: Authorize the supplied public key for the administrator.
# --custom-data: Pass the generated cloud-init provisioning document to the VM.
# --enable-agent: Tell Azure that the guest will not retain the Azure Linux Agent.
# --enable-auto-update: Disable Azure Linux Agent updates because the agent is removed.
# --security-type: Enable the Trusted Launch security profile.
# --enable-secure-boot: Validate signed boot components with Secure Boot.
# --enable-vtpm: Provide the VM with a virtual Trusted Platform Module.
# --os-disk-size-gb: Allocate 32 GiB for the OS and Cargo build artifacts.
# --storage-sku: Use Standard locally redundant storage for the OS disk.
# --os-disk-delete-option: Delete the managed OS disk when the VM is deleted.
# --nics: Attach the prepared private backend NIC instead of creating networking.
# --nic-delete-option: Delete the attached NIC when the VM is deleted.
# --no-wait: Submit the long-running operation without using the faulty CLI poller.
# --output: Suppress the initial operation response.
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
  --nics "$nic_name" \
  --nic-delete-option Delete \
  --no-wait \
  --output none
# Wait until Azure reports that the virtual machine was created successfully.
# --resource-group: Find the VM in this deployment's resource group.
# --name: Select the user-supplied VM name.
# --created: Wait for a successful Created provisioning state.
az vm wait \
  --resource-group "$resource_group" \
  --name "$vm" \
  --created

public_ip="$(
  # Read the address assigned to the load balancer public-IP resource.
  # --resource-group: Find the public IP in the deployment resource group.
  # --name: Select the derived public-IP resource name.
  # --query: Extract only the assigned IPv4 address.
  # --output: Return the address as unquoted tab-separated text.
  az network public-ip show \
    --resource-group "$resource_group" \
    --name "$public_ip_name" \
    --query ipAddress \
    --output tsv
)"

echo
echo "VM deployment completed. First-boot provisioning may still be running."
echo "Resource group: $resource_group"
echo "Load balancer: $load_balancer_name"
echo "Public IP:     $public_ip"
echo "SSH:           ssh ${admin_username}@${public_ip}"
echo
echo "Wait for cloud-init:"
echo "  ssh ${admin_username}@${public_ip} 'sudo cloud-init status --wait --long'"
echo "Follow provisioning output:"
echo "  ssh ${admin_username}@${public_ip} 'sudo tail -f /var/log/cloud-init-output.log'"
