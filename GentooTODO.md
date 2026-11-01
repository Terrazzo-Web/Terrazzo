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

- Architecture: ARM64 (use an ARM64 Ubuntu image)
- VM size: `Standard_D2ps_v5` or larger
- Security type: Standard (simplifies custom bootloaders)
- OS disk: 30 GiB
- Authentication: SSH public key

The selected stage3 is ARM64, musl, and OpenRC. The Ubuntu builder and final Gentoo VM must both use ARM64 for this native installation workflow. On Ubuntu, `uname -m` must report `aarch64` before entering the chroot. An x86-64 builder cannot execute this stage3 directly.

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

The new disk is `/dev/sdb`. Verify this carefully with `lsblk` before partitioning; the commands below erase that disk.

### 3. Partition and mount it

```
DISK=/dev/sdb

sudo apt-get update
sudo apt-get install btrfs-progs dosfstools parted

sudo parted "$DISK" --script \
  mklabel gpt \
  mkpart ESP fat32 1MiB 513MiB \
  set 1 esp on \
  mkpart root btrfs 513MiB 100%
```

Format the EFI partition as FAT32 and the root partition as Btrfs:

```
sudo mkfs.vfat -F32 /dev/sdb1
sudo mkfs.btrfs -L gentoo-root /dev/sdb2

sudo mkdir -p /mnt/gentoo
sudo mount -o compress=zstd /dev/sdb2 /mnt/gentoo
sudo mkdir -p /mnt/gentoo/efi
sudo mount /dev/sdb1 /mnt/gentoo/efi
```

### 4. Install Gentoo stage3

Use the selected [ARM64 musl/OpenRC stage3](https://distfiles.gentoo.org/releases/arm64/autobuilds/20261025T234555Z/stage3-arm64-musl-openrc-20261025T234555Z.tar.xz). Keep the stage3’s musl/OpenRC profile.

After downloading it to Ubuntu and verifying it against Gentoo’s signed DIGESTS, extract it (skip this if already extracted):

```
sudo tar xpf stage3-arm64-musl-openrc-20261025T234555Z.tar.xz \
  -C /mnt/gentoo \
  --xattrs-include='*.*' \
  --numeric-owner
```

### 5. Enter Gentoo

Start tmux on the Ubuntu builder before entering the chroot so compilation continues if SSH disconnects:

```bash
sudo apt-get install -y tmux
tmux new -s gentoo-install
```

Run the following commands inside tmux. Detach with **Ctrl-b**, then **d**. After reconnecting over SSH to Ubuntu, run `tmux attach -t gentoo-install` to recover the session; the chroot and any running emerge command remain inside it. Do not enter another chroot when reattaching.

If you already entered the chroot without tmux, wait for the current emerge command to finish (or interrupt it with Ctrl-C), then run this sequence to return to Ubuntu, start tmux, and re-enter Gentoo:

```bash
exit
sudo apt-get install -y tmux
tmux new -s gentoo-install
sudo chroot /mnt/gentoo /bin/bash
source /etc/profile
```

Keep the existing mounts; do not repeat the mount commands below in that case. Rerun the interrupted emerge command inside tmux. Already installed packages remain installed, but an interrupted package build may need to restart. Starting tmux does not protect a command already running outside it.

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

Use the source-built distribution kernel for this musl installation and enable initramfs generation through dracut:

```
emerge --sync
mkdir -p /etc/portage/package.use
echo 'sys-kernel/installkernel dracut' > /etc/portage/package.use/installkernel
echo 'GRUB_PLATFORMS="efi-64"' >> /etc/portage/make.conf
emerge --ask=n sys-kernel/installkernel sys-kernel/gentoo-kernel sys-fs/btrfs-progs sys-boot/grub net-misc/openssh net-misc/dhcpcd
```

`--ask=n` disables emerge's confirmation prompt, including when `--ask` is set in `EMERGE_DEFAULT_OPTS`; emerge does not use apt's `-y` option. It will still stop for build errors, unmet configuration requirements, or license restrictions. Building the kernel and its dependencies from source can take a long time; tmux keeps the session available across SSH disconnects.

Configure the system:

```
echo 'hostname="terrazzo-vm"' > /etc/conf.d/hostname

rc-update add sshd default
rc-update add dhcpcd default
```

After booting Gentoo, apply the hostname to the running system if needed:

```
hostname terrazzo-vm
```

Skip this command inside the installation chroot, where it would change the Ubuntu builder's running hostname.

Create your SSH user and install their public key.

Find the filesystem UUIDs with `blkid /dev/sdb1 /dev/sdb2` and configure `/etc/fstab` using those values:

```fstab
UUID=<sdb2-filesystem-uuid> /    btrfs compress=zstd,noatime 0 0
UUID=<sdb1-filesystem-uuid> /efi vfat  defaults              0 2
```

This uses the top level of the Btrfs filesystem as `/`, without separate subvolumes. Keep the EFI partition as FAT32 for UEFI boot.

Install GRUB onto the new disk's EFI partition:

```
grub-install \
  --target=arm64-efi \
  --efi-directory=/efi \
  --bootloader-id=Gentoo \
  --removable \
  --no-nvram

grub-mkconfig -o /boot/grub/grub.cfg
```

Ensure the kernel and initramfs include Btrfs support and the Hyper-V storage/network drivers, and configure the serial console. If you change the initramfs configuration, regenerate it before booting Gentoo.

### 7. Boot Gentoo

Exit the chroot and unmount everything, then detach the Gentoo disk from Ubuntu.

From Azure CLI:

```
az vm disk detach \
  -g gentoo-rg \
  --vm-name ubuntu-builder \
  --name gentoo-disk
```

Create a new ARM64 VM (the Dpsv5 series uses Arm processors):

```
az vm create \
  -g gentoo-rg \
  -n gentoo-vm \
  --attach-os-disk gentoo-disk \
  --os-type linux \
  --size Standard_D2ps_v5
```

Azure supports creating VMs from existing specialized OS disks.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://learn.microsoft.com\&sz=32)

Microsoft Learn



Before booting, ensure the VM uses a compatible Gen2/UEFI configuration and that the disk is recognized as an ARM64 Linux OS disk. If Azure rejects a disk originally created as a data disk, create an OS-disk copy with the appropriate metadata rather than changing the Gentoo filesystem.

## What about converting Ubuntu in place?

It is possible, but I would avoid it.

You could extract Gentoo into `/gentoo`, use `pivot_root` or a bootloader entry to switch root filesystems, and eventually delete Ubuntu. However, replacing the running root filesystem is considerably riskier than preparing a second disk.

The second-disk approach also makes it possible to turn the result into a reusable Azure Compute Gallery image later.

One particularly useful option: You can build Gentoo on the Ubuntu VM with a larger, faster VM size, then shut it down and run Gentoo on a smaller, cheaper VM. You only pay for the additional CPU capacity while compiling.

## Reference documentation

- [Azure Dpsv5 ARM VM sizes](https://learn.microsoft.com/azure/virtual-machines/dpsv5-dpdsv5-series)
- [OpenRC service and runlevel management](https://github.com/OpenRC/openrc/blob/master/user-guide.md)
- [Gentoo GRUB EFI 64-bit platform support](https://packages.gentoo.org/useflags/grub_platforms_efi-64)
