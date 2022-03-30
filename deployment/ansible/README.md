Deployment of zfx-subzero nodes from Ansible
-------------------------------------------

_(ansible core 2.12+)_

The main playbook `site.yml` responsible for installing all mandatory tools to be able to run rust executables, and deploys all nodes into hosts, 
listed in inventory. The playbook checks if the hosts have all required tools before attempting to install them, and if the nodes are already running,
then their processes will be killed. 

### Run deployment

The command example to deploy zfx-subzero nodes:
`ansible-playbook site.yml -i inventory/local/hosts -vvv`

where `-i` defines which hosts to use in `inventory/` dir.