# `rover_postcard_protocol`
This crate provides a driver for stm32-based microcontrollers used by the project.

Specifically, it provides an threadsafe, async rust interface to speak to microcontrollers 
via `COBS` encoded `postcard` frames.

```toml
rover_postcard_protocol = {git = "ssh://git@gitlab.com/saddeback_rover_2020/telecom/postcard_protocol.git"}
```

run `cargo doc --open` to view usage documentation.