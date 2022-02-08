# zfx-porter

## Library for mapping ports in NAT setups

Wraps the `rust-igd` library for basic functions to add, refresh and remove port mappings.

**Run `cargo doc --open` for documentation and see examples under `examples`**

## Use

- Construct the `MapperHandler` struct with the local and external addresses and the optional RouterConfig parameter.
- Provide a correct SSDP broadcast address with `RouterConfig` if upnp gateway retrieval is unsuccessful.
- If mapping is successful, it returns the newly mapped entry
- To dinamically refresh port lease, call `refresh_mapping` with the `add_port_mapping` return value and the mapping refresh interval

## TODO

- TODO
