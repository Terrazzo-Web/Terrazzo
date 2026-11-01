Yes. You can start with an Ubuntu Azure VM and replace Ubuntu with Gentoo, without QEMU or uploading a custom VHD.

In fact, for a one-off Gentoo VM, this is probably the easiest approach.

There are two ways to do it:

1. In-place conversion: Extract a Gentoo stage3 into a new root filesystem, configure it, then switch the VM to boot Gentoo. Ubuntu disappears.
2. Second-disk installation (recommended): Use Ubuntu to install Gentoo onto another Azure managed disk, then boot a new VM from that disk.

I recommend the second approach because you can use Ubuntu as a rescue environment if Gentoo fails to boot. It also avoids modifying the original Ubuntu installation.

You can do everything using Azure CLI, SSH, `tar`, `chroot`, and standard Linux utilities.

## Step-by-step: Ubuntu to Gentoo

### 1. Create the Ubuntu VM

In Azure, create an Ubuntu 24.04 LTS VM with:

- Architecture: x86-64
- VM size: `Standard_D2s_v5` or larger
- Security type: Standard (simplifies custom bootloaders)
- OS disk: 30 GiB
- Authentication: SSH public key

### 2. Attach an empty managed disk

From Azure Cloud Shell or your local Azure CLI:

```
az vm disk attach \
  --resource-group gentoo-rg \
  --vm-name ubuntu-builder \
  --name gentoo-disk \
  --new \
  --size-gb 32
```

SSH into Ubuntu and inspect the disks:

```
lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS
```

Assume the new disk is `/dev/sdc`. Verify this carefully; Azure may expose it as an NVMe device instead.

### 3. Partition and mount it

```
DISK=/dev/sdc

sudo parted "$DISK" --script \
  mklabel gpt \
  mkpart ESP fat32 1MiB 513MiB \
  set 1 esp on \
  mkpart root ext4 513MiB 100%
```

For `/dev/sdc`:

```
sudo mkfs.vfat -F32 /dev/sdc1
sudo mkfs.ext4 /dev/sdc2

sudo mkdir -p /mnt/gentoo
sudo mount /dev/sdc2 /mnt/gentoo
sudo mkdir -p /mnt/gentoo/efi
sudo mount /dev/sdc1 /mnt/gentoo/efi
```

### 4. Install Gentoo stage3

Choose your stage3 from [Gentoo downloads](https://www.gentoo.org/downloads/?utm_source=chatgpt.com).

For example, select amd64, systemd, glibc.

Download it to Ubuntu, then extract:

```
sudo tar xpf stage3-amd64-*.tar.xz \
  -C /mnt/gentoo \
  --xattrs-include='*.*' \
  --numeric-owner
```

### 5. Enter Gentoo

```
sudo cp -L /etc/resolv.conf /mnt/gentoo/etc/

sudo mount -t proc proc /mnt/gentoo/proc
sudo mount --rbind /sys /mnt/gentoo/sys
sudo mount --make-rslave /mnt/gentoo/sys
sudo mount --rbind /dev /mnt/gentoo/dev
sudo mount --make-rslave /mnt/gentoo/dev
sudo mount --bind /run /mnt/gentoo/run

sudo chroot /mnt/gentoo /bin/bash
source /etc/profile
```

You are now running Gentoo userspace on Ubuntu's kernel. This is the normal Gentoo installation method.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://emirror.d0a.io\&sz=32)

Gentoo wiki



### 6. Configure Gentoo

Inside the chroot, follow the Gentoo Handbook to initialize Portage and install a kernel.

For example:

```
emerge --sync
emerge sys-kernel/gentoo-kernel-bin
emerge sys-boot/grub
emerge net-misc/openssh
emerge net-misc/dhcpcd
```

Configure the system:

```
echo gentoo-azure > /etc/hostname

systemctl enable sshd
systemctl enable dhcpcd
```

Create your SSH user, install their public key, and configure `/etc/fstab` using filesystem UUIDs.

Install GRUB onto the new disk's EFI partition:

```
grub-install \
  --target=x86_64-efi \
  --efi-directory=/efi \
  --bootloader-id=Gentoo \
  --removable \
  --no-nvram

grub-mkconfig -o /boot/grub/grub.cfg
```

Ensure the kernel and initramfs include the Hyper-V storage/network drivers, and configure the serial console.

### 7. Boot Gentoo

Exit the chroot and unmount everything, then detach the Gentoo disk from Ubuntu.

From Azure CLI:

```
az vm disk detach \
  -g gentoo-rg \
  --vm-name ubuntu-builder \
  --name gentoo-disk
```

Create a new VM:

```
az vm create \
  -g gentoo-rg \
  -n gentoo-vm \
  --attach-os-disk gentoo-disk \
  --os-type linux \
  --size Standard_D2s_v5
```

Azure supports creating VMs from existing specialized OS disks.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://learn.microsoft.com\&sz=32)

Microsoft Learn



Before booting, ensure the VM uses a compatible Gen2/UEFI configuration and that the disk is recognized as a Linux OS disk. If Azure rejects a disk originally created as a data disk, create an OS-disk copy with the appropriate metadata rather than changing the Gentoo filesystem.

## What about converting Ubuntu in place?

It is possible, but I would avoid it.

You could extract Gentoo into `/gentoo`, use `pivot_root` or a bootloader entry to switch root filesystems, and eventually delete Ubuntu. However, replacing the running root filesystem is considerably riskier than preparing a second disk.

The second-disk approach also makes it possible to turn the result into a reusable Azure Compute Gallery image later.

One particularly useful option: You can build Gentoo on the Ubuntu VM with a larger, faster VM size, then shut it down and run Gentoo on a smaller, cheaper VM. You only pay for the additional CPU capacity while compiling.
