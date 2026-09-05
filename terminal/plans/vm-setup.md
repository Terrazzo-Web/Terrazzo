TODO: Update this file with a plan to create a script that creates an azure VM
- output of the script is a shell script
- assume you are already az login
- name of the vm $vm is the first parameter to the script
- every resource must be created in a resource group called ${vm}-rg
- goal is to have an azure VM the most lightweight possible OS.
- Use SKU Standard D2ps v6 (2 vcpus, 8 GiB memory)
- in Germany West Central
- no azure agent
- no password: only way to login is ssh with my private key to user 'richard' in the VM
- richard in the Alpine VM can run sudo without typing his password (also he has no password)
- then install rust compiler and terrazzo-terminal (and terrazzo-terminal's prereqs, protoc, wasm and terrazzo-cli)
- open port 80 and 443 (and 22 for ssh)
- 

This script is what I used to setup the Ubuntu VM, but you need to translate for Alpine
```
echo 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBaRBRd4kNHeFgqxi09MlCNmf6DT4ELjoDDWNXqJUuJp Richard @ MacBook Air' > $HOME/.ssh/authorized_keys
sudo passwd -dl $USER

time sudo apt update
time sudo apt dist-upgrade
time sudo apt install -y build-essential pkg-config musl-tools libssl-dev protobuf-compiler nano

time curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

-- source the env file so rustup and cargo are on PATH

time rustup update
time rustup target add x86_64-unknown-linux-musl
time rustup target add wasm32-unknown-unknown

time cargo install --locked --force wasm-pack
time cargo install --locked terrazzo-css-cli
time cargo install --locked terrazzo-terminal --features prod
sudo setcap 'cap_net_bind_service=+ep' ~/.cargo/bin/terrazzo-terminal
```

